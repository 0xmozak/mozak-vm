#![deny(clippy::pedantic)]
#![deny(clippy::cargo)]
// TODO(bing): `clio` uses an older `windows-sys` vs other dependencies.
// Remove when `clio` updates, or if `clio` is no longer needed.
#![allow(clippy::multiple_crate_versions)]
use std::collections::{BTreeSet, HashMap};
use std::io::{Read, Write};
use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use clap_derive::Args;
use clio::{Input, Output};
use itertools::{izip, Itertools};
use log::debug;
use mozak_circuits::generation::memoryinit::generate_elf_memory_init_trace;
use mozak_circuits::generation::storage_device::generate_call_tape_trace;
use mozak_circuits::program::generation::generate_program_rom_trace;
use mozak_circuits::stark::mozak_stark::{MozakStark, PublicInputs};
use mozak_circuits::stark::proof::AllProof;
use mozak_circuits::stark::prover::prove;
use mozak_circuits::stark::recursive_verifier::{
    circuit_data_for_recursion, recursive_mozak_stark_circuit,
    shrink_to_target_degree_bits_circuit, VM_PUBLIC_INPUT_SIZE, VM_RECURSION_CONFIG,
    VM_RECURSION_THRESHOLD_DEGREE_BITS,
};
use mozak_circuits::stark::utils::trace_rows_to_poly_values;
use mozak_circuits::stark::verifier::verify_proof;
use mozak_circuits::test_utils::{prove_and_verify_mozak_stark, C, D, F, S};
use mozak_cli::cli_benches::benches::BenchArgs;
use mozak_cli::runner::{deserialize_system_tape, load_program};
use mozak_node::types::{Attestation, Transaction};
use mozak_runner::state::{RawTapes, State};
use mozak_runner::vm::step;
use mozak_sdk::common::merkle::merkleize;
use mozak_sdk::common::types::{
    CanonicalOrderedTemporalHints, CrossProgramCall, Poseidon2Hash, ProgramIdentifier, SystemTape,
};
use plonky2::field::types::Field;
use plonky2::fri::oracle::PolynomialBatch;
use plonky2::plonk::circuit_data::VerifierOnlyCircuitData;
use plonky2::plonk::proof::ProofWithPublicInputs;
use plonky2::util::timing::TimingTree;
use rkyv::rancor::{Panic, Strategy};
use rkyv::ser::AllocSerializer;
use starky::config::StarkConfig;

const PROGRAMS_MAP_JSON: &str = "examples/programs_map.json";

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[clap(flatten)]
    verbose: clap_verbosity_flag::Verbosity,
    #[command(subcommand)]
    command: Command,
    /// Debug API, default is OFF, currently only `prove` command is supported
    #[arg(short, long)]
    debug: bool,
}

#[derive(Clone, Debug, Args)]
pub struct RunArgs {
    elf: Input,
    #[arg(long)]
    system_tape: Option<Input>,
    #[arg(long)]
    self_prog_id: Option<String>,
}

#[derive(Clone, Debug, Args)]
pub struct ProveArgs {
    elf: Input,
    proof: Output,
    #[arg(long)]
    system_tape: Option<Input>,
    #[arg(long)]
    self_prog_id: Option<String>,
    recursive_proof: Option<Output>,
}

#[derive(Clone, Debug, Subcommand)]
enum Command {
    /// Decode a given ELF and prints the program
    Decode { elf: Input },
    /// Decode and execute a given ELF. Prints the final state of
    /// the registers
    Run(RunArgs),
    /// Prove and verify the execution of a given ELF
    ProveAndVerify(RunArgs),
    /// Prove the execution of given ELF and write proof to file.
    Prove(ProveArgs),
    /// Verify the given proof from file.
    Verify { proof: Input },
    /// Verify the given recursive proof from file.
    VerifyRecursiveProof { proof: Input, verifier_key: Input },
    /// Builds a transaction bundle.
    BundleTransaction {
        /// System tape generated from native execution.
        #[arg(long, required = true)]
        system_tape: Input,
        /// Output file path of the serialized bundle.
        #[arg(long, default_value = "bundle")]
        bundle: Output,
    },
    /// Compute the Program Rom Hash of the given ELF.
    ProgramRomHash { elf: Input },
    /// Compute the Memory Init Hash of the given ELF.
    MemoryInitHash { elf: Input },
    /// Bench the function with given parameters
    Bench(BenchArgs),
}

fn length_prefixed_bytes(data: Vec<u8>, dgb_string: &str) -> Vec<u8> {
    let data_len = data.len();
    let mut len_prefix_bytes = Vec::with_capacity(data_len + 4);
    len_prefix_bytes.extend_from_slice(
        &(u32::try_from(data.len()))
            .expect("length of data's max size shouldn't be more than u32")
            .to_le_bytes(),
    );
    len_prefix_bytes.extend(data);
    debug!(
        "Length-Prefixed {:<15} of byte len: {:>5}, on-mem bytes: {:>5}",
        dgb_string,
        data_len,
        len_prefix_bytes.len()
    );
    len_prefix_bytes
}

fn raw_tapes_from_system_tape(
    sys: &SystemTape,
    self_prog_id: ProgramIdentifier,
) -> Result<RawTapes> {
    fn serialise<T>(tape: &T, dgb_string: &str) -> Vec<u8>
    where
        T: rkyv::Archive + rkyv::Serialize<Strategy<AllocSerializer<256>, Panic>>, {
        let tape_bytes = rkyv::to_bytes::<_, 256, _>(tape).unwrap().into();
        length_prefixed_bytes(tape_bytes, dgb_string)
    }

    let cast_list = sys
        .call_tape
        .writer
        .iter()
        .map(|msg| msg.callee)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect_vec();

    let canonical_order_temporal_hints: Vec<CanonicalOrderedTemporalHints> = sys
        .event_tape
        .writer
        .get(&self_prog_id)
        .cloned()
        .unwrap_or_default()
        .get_canonical_order_temporal_hints();

    let events_commitment_tape = merkleize(
        canonical_order_temporal_hints
            .iter()
            .map(|x| {
                (
                    // May not be the best idea if
                    // `addr` > goldilock's prime, cc
                    // @Kapil
                    u64::from_le_bytes(x.0.address.inner()),
                    x.0.canonical_hash(),
                )
            })
            .collect::<Vec<(u64, Poseidon2Hash)>>(),
    )
    .0;

    let cast_list_commitment_tape =
        merkleize(izip!(0.., &cast_list).map(|(idx, x)| (idx, x.0)).collect()).0;

    println!("Self Prog ID: {self_prog_id:#?}");
    println!("Found events: {:#?}", canonical_order_temporal_hints.len());

    Ok(RawTapes {
        private_tape: length_prefixed_bytes(
            sys.private_input_tape
                .writer
                .get(&self_prog_id)
                .cloned()
                .unwrap_or_default()
                .0,
            "PRIVATE_TAPE",
        ),
        public_tape: length_prefixed_bytes(
            sys.public_input_tape
                .writer
                .get(&self_prog_id)
                .cloned()
                .unwrap_or_default()
                .0,
            "PUBLIC_TAPE",
        ),
        call_tape: serialise(&sys.call_tape.writer, "CALL_TAPE"),
        event_tape: serialise(&canonical_order_temporal_hints, "EVENT_TAPE"),
        self_prog_id_tape: self_prog_id.0 .0,
        events_commitment_tape,
        cast_list_commitment_tape,
    })
}

/// Run me eg like `cargo run -- -vvv run vm/tests/testdata/rv32ui-p-addi
/// iotape.txt`
#[allow(clippy::too_many_lines)]
fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = StarkConfig::standard_fast_config();
    env_logger::Builder::new()
        .filter_level(cli.verbose.log_level_filter())
        .init();
    match cli.command {
        Command::Decode { elf } => {
            let program = load_program(elf)?;
            debug!("{program:?}");
        }
        Command::Run(RunArgs {
            elf,
            system_tape,
            self_prog_id,
        }) => {
            let raw_tapes = system_tape.map_or_else(RawTapes::default, |s| {
                raw_tapes_from_system_tape(
                    &deserialize_system_tape(s).unwrap(),
                    self_prog_id.unwrap().into(),
                )
                .unwrap_or_default()
            });
            let program = load_program(elf).unwrap();
            let state: State<F> = State::new(program.clone(), raw_tapes);
            step(&program, state)?;
        }
        Command::ProveAndVerify(RunArgs {
            elf,
            system_tape,
            self_prog_id,
        }) => {
            let program = load_program(elf).unwrap();
            let raw_tapes = system_tape.map_or_else(RawTapes::default, |s| {
                raw_tapes_from_system_tape(
                    &deserialize_system_tape(s).unwrap(),
                    self_prog_id.unwrap().into(),
                )
                .unwrap_or_default()
            });

            let state = State::new(program.clone(), raw_tapes);
            let record = step(&program, state)?;
            prove_and_verify_mozak_stark(&program, &record, &config)?;
        }
        Command::Prove(ProveArgs {
            elf,
            system_tape,
            self_prog_id,
            mut proof,
            recursive_proof,
        }) => {
            let program = load_program(elf).unwrap();
            let raw_tapes = system_tape.map_or_else(RawTapes::default, |s| {
                raw_tapes_from_system_tape(
                    &deserialize_system_tape(s).unwrap(),
                    self_prog_id.unwrap().into(),
                )
                .unwrap_or_default()
            });

            let state = State::new(program.clone(), raw_tapes);
            let record = step(&program, state)?;
            let stark = if cli.debug {
                MozakStark::default_debug()
            } else {
                MozakStark::default()
            };
            let public_inputs = PublicInputs {
                entry_point: F::from_canonical_u32(program.entry_point),
            };
            let all_proof = prove::<F, C, D>(
                &program,
                &record,
                &stark,
                &config,
                public_inputs,
                &mut TimingTree::default(),
            )?;

            let serialized = serde_json::to_string(&all_proof).unwrap();
            proof.write_all(serialized.as_bytes())?;

            // Generate recursive proof
            if let Some(mut recursive_proof_output) = recursive_proof {
                let degree_bits = all_proof.degree_bits(&config);
                let recursive_circuit = recursive_mozak_stark_circuit::<F, C, D>(
                    &stark,
                    &degree_bits,
                    &VM_RECURSION_CONFIG,
                    &config,
                );

                let recursive_all_proof = recursive_circuit.prove(&all_proof)?;

                let (final_circuit, final_proof) = shrink_to_target_degree_bits_circuit(
                    &recursive_circuit.circuit,
                    &VM_RECURSION_CONFIG,
                    VM_RECURSION_THRESHOLD_DEGREE_BITS,
                    &recursive_all_proof,
                )?;
                assert_eq!(
                    final_circuit.circuit.common.num_public_inputs,
                    VM_PUBLIC_INPUT_SIZE
                );

                let s = final_proof.to_bytes();
                recursive_proof_output.write_all(&s)?;

                // Generate the verifier key file
                let mut vk_output_path = recursive_proof_output.path().clone();
                vk_output_path.set_extension("vk");
                let mut vk_output = vk_output_path.create()?;

                let bytes = final_circuit.circuit.verifier_only.to_bytes().unwrap();
                vk_output.write_all(&bytes)?;
            }

            debug!("proof generated successfully!");
        }
        Command::BundleTransaction {
            system_tape: system_tape_path,
            bundle,
        } => {
            /// Returns mapping of program ID to elf path.
            ///
            /// The first entry is always the entrypoint program.
            fn ids_and_paths_from_cast_list(
                entrypoint_program_id: ProgramIdentifier,
                cast_list: &[ProgramIdentifier],
            ) -> Vec<(ProgramIdentifier, PathBuf)> {
                /// A `MappedProgram` is a (name, path) tuple of a `MozakVM`
                /// program, where the name
                /// is the [`ProgramIdentifier`] and the path is the expected
                /// path of the compiled `MozakVM` binary,
                /// relative to the examples directory.
                #[derive(serde::Deserialize, serde::Serialize)]
                struct MappedProgram {
                    name: String,
                    path: String,
                }

                let curr_dir = std::env::current_dir().unwrap();
                let mapping = std::fs::File::open(curr_dir.join(PROGRAMS_MAP_JSON))
                    .expect("could not open programs map");
                let mapping: Vec<MappedProgram> = serde_json::from_reader(mapping)
                    .expect("Could not deserialize Vec<MappedProgram> from programs map");
                let mapping: HashMap<ProgramIdentifier, String> = mapping
                    .into_iter()
                    .map(|mp| (ProgramIdentifier::from(mp.name), mp.path))
                    .collect();
                cast_list
                    .iter()
                    .filter_map(|id: &ProgramIdentifier| {
                        mapping.get(id).map(|path| (*id, curr_dir.join(path)))
                    })
                    .sorted_by_key(|(id, _)| id != &entrypoint_program_id)
                    .collect()
            }

            println!("Bundling transaction...");

            let system_tape: SystemTape = deserialize_system_tape(system_tape_path.clone())?;

            // Q: will first call always be null program calling the program's entrypoint?
            let entrypoint_program_id = system_tape.call_tape.writer[0].callee;

            let cast_list: Vec<_> = system_tape
                .call_tape
                .writer
                .clone()
                .into_iter()
                .flat_map(|CrossProgramCall { callee, caller, .. }| [callee, caller])
                .filter(|prog| !prog.is_null_program())
                .sorted()
                .dedup()
                .collect();

            let ids_and_paths = ids_and_paths_from_cast_list(entrypoint_program_id, &cast_list);

            // This does nothing - we rely entirely on ecalls.
            // TODO(bing): Refactor `load_program` to not take this as a param, in a
            // separate PR.

            let mut attestations: Vec<Attestation> = vec![];
            let mut call_tape_hash = None;

            for (i, (program_id, elf)) in ids_and_paths.iter().enumerate() {
                let program = load_program(
                    Input::try_from(elf)
                        .unwrap_or_else(|_| panic!("Elf filepath {elf:?} not found")),
                )?;

                let raw_call_tape: Vec<u8> =
                    serde_json::to_vec(&system_tape.call_tape.writer).unwrap();
                let raw_tapes = RawTapes {
                    call_tape: raw_call_tape,
                    private_tape: system_tape
                        .private_input_tape
                        .writer
                        .get(program_id)
                        .cloned()
                        .unwrap_or_default()
                        .to_vec(),
                    ..Default::default()
                };
                if i == 0 {
                    let rate_bits = config.fri_config.rate_bits;
                    let cap_height = config.fri_config.cap_height;

                    let state: State<F> = State::new(program.clone(), raw_tapes.clone());
                    let record =
                        step(&program, state).expect("Could not step through the given program");

                    let trace = generate_call_tape_trace(&record.executed);
                    let poly_values = trace_rows_to_poly_values(trace);

                    let trace_commitment = PolynomialBatch::<F, C, D>::from_values(
                        poly_values,
                        rate_bits,
                        false, // blinding
                        cap_height,
                        &mut TimingTree::default(),
                        None, // fft_root_table
                    );

                    call_tape_hash = Some(trace_commitment.merkle_tree.cap);
                }

                let attestation = Attestation {
                    id: *program_id,
                    public_tape: system_tape
                        .public_input_tape
                        .writer
                        .get(program_id)
                        .cloned()
                        .unwrap_or_default()
                        .to_vec(),
                    event_tape: system_tape
                        .event_tape
                        .writer
                        .get(program_id)
                        .cloned()
                        .unwrap_or_default(),
                };

                attestations.push(attestation);
            }

            let transaction: Transaction<F, C, D> = Transaction {
                call_tape_hash: call_tape_hash.expect("system tape generated from entrypoint program's native execution should contain a call tape"),
                cast_list,
                constituent_zs: attestations,
            };

            serde_json::to_writer_pretty(bundle, &transaction)?;
            println!("Transaction bundled: {transaction:?}");
        }

        Command::Verify { mut proof } => {
            let stark = S::default();
            let mut buffer: Vec<u8> = vec![];
            proof.read_to_end(&mut buffer)?;
            let all_proof: AllProof<F, C, D> = serde_json::from_slice(&buffer)?;
            verify_proof(&stark, all_proof, &config)?;
            println!("proof verified successfully!");
        }
        Command::VerifyRecursiveProof {
            mut proof,
            mut verifier_key,
        } => {
            let mut circuit = circuit_data_for_recursion::<F, C, D>(
                &VM_RECURSION_CONFIG,
                VM_RECURSION_THRESHOLD_DEGREE_BITS,
                VM_PUBLIC_INPUT_SIZE,
            );

            let mut vk_buffer: Vec<u8> = vec![];
            verifier_key.read_to_end(&mut vk_buffer)?;
            circuit.verifier_only = VerifierOnlyCircuitData::from_bytes(vk_buffer).unwrap();

            let mut proof_buffer: Vec<u8> = vec![];
            proof.read_to_end(&mut proof_buffer)?;
            let proof: ProofWithPublicInputs<F, C, D> =
                ProofWithPublicInputs::from_bytes(proof_buffer, &circuit.common).map_err(|_| {
                    anyhow::Error::msg("ProofWithPublicInputs deserialization failed.")
                })?;
            println!("Public Inputs: {:?}", proof.public_inputs);
            println!("Verifier Key: {:?}", circuit.verifier_only);

            circuit.verify(proof.clone())?;
            println!("Recursive VM proof verified successfully!");
        }
        Command::ProgramRomHash { elf } => {
            let program = load_program(elf)?;
            let trace = generate_program_rom_trace(&program);
            let trace_poly_values = trace_rows_to_poly_values(trace);
            let rate_bits = config.fri_config.rate_bits;
            let cap_height = config.fri_config.cap_height;
            let trace_commitment = PolynomialBatch::<F, C, D>::from_values(
                trace_poly_values,
                rate_bits,
                false, // blinding
                cap_height,
                &mut TimingTree::default(),
                None, // fft_root_table
            );
            let trace_cap = trace_commitment.merkle_tree.cap;
            println!("{trace_cap:?}");
        }
        Command::MemoryInitHash { elf } => {
            let program = load_program(elf)?;
            let trace = generate_elf_memory_init_trace(&program);
            let trace_poly_values = trace_rows_to_poly_values(trace);
            let rate_bits = config.fri_config.rate_bits;
            let cap_height = config.fri_config.cap_height;
            let trace_commitment = PolynomialBatch::<F, C, D>::from_values(
                trace_poly_values,
                rate_bits,
                false, // blinding
                cap_height,
                &mut TimingTree::default(),
                None, // fft_root_table
            );
            let trace_cap = trace_commitment.merkle_tree.cap;
            println!("{trace_cap:?}");
        }
        Command::Bench(bench) => {
            let time_taken = bench.bench()?.as_secs_f64();
            println!("{time_taken}");
        }
    }
    Ok(())
}

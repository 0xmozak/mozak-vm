#![deny(clippy::pedantic)]
#![deny(clippy::cargo)]
// TODO: remove this when shadow_rs updates enough.
#![allow(clippy::needless_raw_string_hashes)]
use std::io::{Read, Write};
use std::time::Duration;

use anyhow::Result;
use clap::{Parser, Subcommand};
use clap_derive::Args;
use clio::{Input, Output};
use log::{debug, warn};
use mozak_circuits::generation::memoryinit::generate_elf_memory_init_trace;
use mozak_circuits::generation::program::generate_program_rom_trace;
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
use mozak_runner::elf::Program;
use mozak_runner::state::State;
use mozak_runner::vm::step;
use mozak_sdk::coretypes::{Event, ProgramIdentifier};
use mozak_sdk::sys::{EventTape, SystemTapes};
use plonky2::field::goldilocks_field::GoldilocksField;
use plonky2::field::types::Field;
use plonky2::fri::oracle::PolynomialBatch;
use plonky2::plonk::circuit_data::VerifierOnlyCircuitData;
use plonky2::plonk::proof::ProofWithPublicInputs;
use plonky2::util::timing::TimingTree;
use rkyv::Deserialize;
use starky::config::StarkConfig;

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
pub struct RuntimeArguments {
    #[arg(long)]
    self_prog_id: Option<Input>,
    /// Private input.
    #[arg(long)]
    io_tape_private: Option<Input>,
    /// Public input.
    #[arg(long)]
    io_tape_public: Option<Input>,
    #[arg(long)]
    call_tape: Option<Input>,
    #[arg(long)]
    event_tape: Option<Input>,
}

#[derive(Clone, Debug, Args)]
pub struct RunArgs {
    elf: Input,
    #[arg(long)]
    system_tape: Option<Input>,
    #[arg(long)]
    self_prog_id: String,
}

#[derive(Clone, Debug, Args)]
pub struct ProveArgs {
    elf: Input,
    proof: Output,
    #[arg(long)]
    system_tape: Option<Input>,
    #[arg(long)]
    self_prog_id: String,
    recursive_proof: Option<Output>,
}

impl From<RuntimeArguments> for mozak_runner::elf::RuntimeArguments {
    fn from(value: RuntimeArguments) -> Self {
        let mut self_prog_id = ProgramIdentifier::default();
        let mut io_tape_private = vec![];
        let mut io_tape_public = vec![];
        let mut call_tape = vec![];
        let mut event_tape = vec![];

        if let Some(t) = value.self_prog_id {
            self_prog_id = t.to_string().into();
        }

        if let Some(mut t) = value.io_tape_private {
            let bytes_read = t
                .read_to_end(&mut io_tape_private)
                .expect("Read should pass");
            debug!("Read {bytes_read} of io_tape data.");
        };

        if let Some(mut t) = value.io_tape_public {
            let bytes_read = t
                .read_to_end(&mut io_tape_public)
                .expect("Read should pass");
            debug!("Read {bytes_read} of io_tape data.");
        };

        if let Some(mut t) = value.call_tape {
            let bytes_read = t.read_to_end(&mut call_tape).expect("Read should pass");
            debug!("Read {bytes_read} of call_tape data.");
        };

        if let Some(mut t) = value.event_tape {
            let bytes_read = t.read_to_end(&mut event_tape).expect("Read should pass");
            debug!("Read {bytes_read} of event_tape data.");
        };

        Self {
            self_prog_id: self_prog_id.to_le_bytes().to_vec(),
            cast_list: vec![], // will be populated later when `event_tape` is parsed
            io_tape_private,
            io_tape_public,
            call_tape,
            event_tape,
        }
    }
}

/// Deserializes an rkyv-serialized system tape binary file into `SystemTapes`.
///
/// # Errors
///
/// Errors if reading from the binary file fails.
///
/// # Panics
///
/// Panics if deserialization fails.
pub fn deserialize_system_tape(mut bin: Input) -> Result<SystemTapes> {
    let mut sys_tapes_bytes = Vec::new();
    let bytes_read = bin.read_to_end(&mut sys_tapes_bytes)?;
    debug!("Read {bytes_read} of system tape data.");
    let sys_tapes_archived = unsafe { rkyv::archived_root::<SystemTapes>(&sys_tapes_bytes[..]) };
    let deserialized: SystemTapes = sys_tapes_archived
        .deserialize(&mut rkyv::Infallible)
        .unwrap();
    Ok(deserialized)
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

/// Deserializes an rkyv-serialized system tape binary file into
/// [`SystemTapes`](mozak_sdk::sys::SystemTapes).
///
/// # Panics
///
/// Panics if conversion from rkyv-serialized system tape to
/// [`RuntimeArguments`](mozak_runner::elf::RuntimeArguments)
/// fails.
pub fn tapes_to_runtime_arguments(
    tape_bin: Input,
    self_prog_id: String,
) -> mozak_runner::elf::RuntimeArguments {
    let sys_tapes: SystemTapes = deserialize_system_tape(tape_bin).unwrap();
    let self_prog_id: ProgramIdentifier = self_prog_id.into();
    let mut cast_list: Vec<ProgramIdentifier> =
        Vec::with_capacity(sys_tapes.event_tape.writer.len());
    let mut event_tape_single: Option<&Vec<Event>> = None;
    for single_tape in &sys_tapes.event_tape.writer {
        cast_list.push(single_tape.id);
        if single_tape.id == self_prog_id {
            event_tape_single = Some(&single_tape.contents);
        }
    }
    if event_tape_single.is_none() {
        warn!("event tape not found in bundle. Proving may not work as intended.");
    }
    cast_list.sort();

    debug!("Self Prog ID: {self_prog_id:#?}");
    debug!("Cast List (canonical repr): {cast_list:#?}");

    mozak_runner::elf::RuntimeArguments::default()
    // {
    //     self_prog_id: self_prog_id.to_le_bytes().to_vec(),
    //     cast_list: length_prefixed_bytes(
    //         rkyv::to_bytes::<_, 256>(&cast_list).unwrap().into(),
    //         "CAST_LIST",
    //     ),
    //     io_tape_public: length_prefixed_bytes(
    //         rkyv::to_bytes::<_, 256>(&sys_tapes.public_tape)
    //             .unwrap()
    //             .into(),
    //         "IO_TAPE_PUBLIC",
    //     ),
    //     io_tape_private: length_prefixed_bytes(
    //         rkyv::to_bytes::<_, 256>(&sys_tapes.private_tape)
    //             .unwrap()
    //             .into(),
    //         "IO_TAPE_PRIVATE",
    //     ),
    //     call_tape: length_prefixed_bytes(
    //         rkyv::to_bytes::<_, 256>(&sys_tapes.call_tape.writer)
    //             .unwrap()
    //             .into(),
    //         "CALL_TAPE",
    //     ),
    //     event_tape: length_prefixed_bytes(
    //         rkyv::to_bytes::<_, 256>(event_tape_single.unwrap())
    //             .unwrap()
    //             .into(),
    //         "EVENT_TAPE",
    //     ),
    // }
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
    /// Compute the Program Rom Hash of the given ELF.
    ProgramRomHash { elf: Input },
    /// Compute the Memory Init Hash of the given ELF.
    MemoryInitHash { elf: Input },
    /// Deserialize a `SystemTape` from a binary. Useful for debugging.
    DeserializeTape { tapes: Input },
    /// Bench the function with given parameters
    Bench(BenchArgs),
}

fn load_program(mut elf: Input) -> Result<Program> {
    let mut elf_bytes = Vec::new();
    let bytes_read = elf.read_to_end(&mut elf_bytes)?;
    debug!("Read {bytes_read} of ELF data.");
    Program::vanilla_load_elf(&elf_bytes)
}

fn load_program_with_args(
    mut elf: Input,
    args: &mozak_runner::elf::RuntimeArguments,
) -> Result<Program> {
    let mut elf_bytes = Vec::new();
    let bytes_read = elf.read_to_end(&mut elf_bytes)?;
    debug!("Read {bytes_read} of ELF data.");

    Program::mozak_load_program(&elf_bytes, args)
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
            let args = tapes_to_runtime_arguments(system_tape.unwrap(), self_prog_id);
            let program = load_program_with_args(elf, &args).unwrap();
            let state = State::<GoldilocksField>::legacy_ecall_api_new(program.clone(), args);
            let _state = step(&program, state)?.last_state;
        }
        Command::ProveAndVerify(RunArgs {
            elf,
            system_tape,
            self_prog_id,
        }) => {
            let args = tapes_to_runtime_arguments(system_tape.unwrap(), self_prog_id);

            let program = load_program_with_args(elf, &args).unwrap();
            let state = State::<GoldilocksField>::legacy_ecall_api_new(program.clone(), args);

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
            let args = system_tape.map_or_else(mozak_runner::elf::RuntimeArguments::default, |s| {
                tapes_to_runtime_arguments(s, self_prog_id)
            });
            let program = load_program_with_args(elf, &args).unwrap();
            let state = State::<GoldilocksField>::legacy_ecall_api_new(program.clone(), args);
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
            let s = all_proof.serialize_proof_to_flexbuffer()?;
            proof.write_all(s.view())?;

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
        Command::Verify { mut proof } => {
            let stark = S::default();
            let mut buffer: Vec<u8> = vec![];
            proof.read_to_end(&mut buffer)?;
            let all_proof = AllProof::<F, C, D>::deserialize_proof_from_flexbuffer(&buffer)?;
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
        Command::DeserializeTape { tapes } => {
            let sys_tapes: SystemTapes = deserialize_system_tape(tapes)?;
            println!("{sys_tapes:?}");
        }
        Command::Bench(bench) => {
            let time_taken = timeit(&|| bench.run())?.as_secs_f64();
            println!("{time_taken}");
        }
    }
    Ok(())
}

/// Times a function and returns the `Duration`.
///
/// # Errors
///
/// This errors if the given function returns an `Err`.
pub fn timeit(func: &impl Fn() -> Result<()>) -> Result<Duration> {
    let start_time = std::time::Instant::now();
    func()?;
    Ok(start_time.elapsed())
}

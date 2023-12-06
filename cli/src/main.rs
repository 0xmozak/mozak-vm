#![deny(clippy::pedantic)]
#![deny(clippy::cargo)]
// TODO: remove this when shadow_rs updates enough.
#![allow(clippy::needless_raw_string_hashes)]
use std::io::{Read, Write};
use std::time::Duration;

use anyhow::Result;
use clap::{Parser, Subcommand};
use clio::{Input, Output};
use log::debug;
use mozak_circuits::generation::memoryinit::generate_memory_init_trace;
use mozak_circuits::generation::program::generate_program_rom_trace;
use mozak_circuits::stark::mozak_stark::{MozakStark, PublicInputs, TableKindArray};
use mozak_circuits::stark::proof::AllProof;
use mozak_circuits::stark::prover::prove;
use mozak_circuits::stark::recursive_verifier::recursive_mozak_stark_circuit;
use mozak_circuits::stark::utils::trace_rows_to_poly_values;
use mozak_circuits::stark::verifier::verify_proof;
use mozak_circuits::test_utils::{prove_and_verify_mozak_stark, C, D, F, S};
use mozak_cli::cli_benches::benches::BenchArgs;
use mozak_runner::elf::Program;
use mozak_runner::state::State;
use mozak_runner::vm::step;
use plonky2::field::goldilocks_field::GoldilocksField;
use plonky2::field::types::Field;
use plonky2::fri::oracle::PolynomialBatch;
use plonky2::plonk::circuit_data::{CircuitConfig, CommonCircuitData};
use plonky2::plonk::proof::ProofWithPublicInputs;
use plonky2::util::serialization::DefaultGateSerializer;
use plonky2::util::timing::TimingTree;
use serde::{Deserialize, Serialize};
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

#[derive(Clone, Debug, Subcommand)]
enum Command {
    /// Decode a given ELF and prints the program
    Decode { elf: Input },
    /// Decode and execute a given ELF. Prints the final state of
    /// the registers
    Run {
        elf: Input,
        io_tape_private: Input,
        io_tape_public: Input,
    },
    /// Prove and verify the execution of a given ELF
    ProveAndVerify {
        elf: Input,
        io_tape_private: Input,
        io_tape_public: Input,
    },
    /// Prove the execution of given ELF and write proof to file.
    Prove {
        elf: Input,
        io_tape_private: Input,
        io_tape_public: Input,
        proof: Output,
        recursive_proof: Option<Output>,
    },
    /// Verify the given proof from file.
    Verify { proof: Input },
    /// Verify the given recursive proof from file.
    VerifyRecursiveProof { proof: Input, common_data: Input },
    /// Compute the Program Rom Hash of the given ELF.
    ProgramRomHash { elf: Input },
    /// Compute the Memory Init Hash of the given ELF.
    MemoryInitHash { elf: Input },
    /// Bench the function with given parameters
    Bench(BenchArgs),
}

/// Read a sequence of bytes from IO
fn load_tape(mut io_tape: impl Read) -> Result<Vec<u8>> {
    let mut io_tape_bytes = Vec::new();
    let bytes_read = io_tape.read_to_end(&mut io_tape_bytes)?;
    debug!("Read {bytes_read} of io_tape data.");
    Ok(io_tape_bytes)
}

fn load_program(mut elf: Input) -> Result<Program> {
    let mut elf_bytes = Vec::new();
    let bytes_read = elf.read_to_end(&mut elf_bytes)?;
    debug!("Read {bytes_read} of ELF data.");
    Program::load_elf(&elf_bytes)
}

#[derive(Serialize, Deserialize)]
struct CommonDataWithDegreeBits {
    common_data_bytes: Vec<u8>,
    degree_bits: TableKindArray<usize>,
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
        Command::Run {
            elf,
            io_tape_private,
            io_tape_public,
        } => {
            let program = load_program(elf)?;
            let state = State::<GoldilocksField>::new(
                program.clone(),
                &load_tape(io_tape_private)?,
                &load_tape(io_tape_public)?,
            );
            let state = step(&program, state)?.last_state;
            debug!("{:?}", state.registers);
        }
        Command::ProveAndVerify {
            elf,
            io_tape_private,
            io_tape_public,
        } => {
            let program = load_program(elf)?;
            let state = State::<GoldilocksField>::new(
                program.clone(),
                &load_tape(io_tape_private)?,
                &load_tape(io_tape_public)?,
            );
            let record = step(&program, state)?;
            prove_and_verify_mozak_stark(&program, &record, &config)?;
        }
        Command::Prove {
            elf,
            io_tape_private,
            io_tape_public,
            mut proof,
            recursive_proof,
        } => {
            let program = load_program(elf)?;
            let state = State::<GoldilocksField>::new(
                program.clone(),
                &load_tape(io_tape_private)?,
                &load_tape(io_tape_public)?,
            );
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
                let circuit_config = CircuitConfig::standard_recursion_config();
                let recursive_circuit = recursive_mozak_stark_circuit::<F, C, D>(
                    &stark,
                    &all_proof.degree_bits(&config),
                    &circuit_config,
                    &config,
                    12,
                );

                let recursive_all_proof = recursive_circuit.prove(&all_proof)?;
                let s = recursive_all_proof.to_bytes();
                recursive_proof_output.write_all(&s)?;

                // Generate the common data file
                let mut cd_output_path = recursive_proof_output.path().clone();
                cd_output_path.set_extension("cd");
                let mut cd_output = cd_output_path.create()?;

                let gate_serializer = DefaultGateSerializer;
                let common_data_bytes = recursive_circuit
                    .circuit
                    .common
                    .to_bytes(&gate_serializer)
                    .map_err(|_| anyhow::Error::msg("CommonCircuitData serialization failed."))?;
                debug!(
                    "Common circuit data length: {} bytes",
                    common_data_bytes.len()
                );

                let common_data_with_degree_bits = CommonDataWithDegreeBits {
                    common_data_bytes,
                    degree_bits: all_proof.degree_bits(&config),
                };

                let serialized = serde_json::to_string(&common_data_with_degree_bits)?;
                cd_output.write_all(serialized.as_bytes())?;
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
            mut common_data,
        } => {
            let mut cd_buffer: Vec<u8> = vec![];
            common_data.read_to_end(&mut cd_buffer)?;

            let deserialized: CommonDataWithDegreeBits = serde_json::from_slice(&cd_buffer)?;
            let gate_serializer = DefaultGateSerializer;
            let common_data = CommonCircuitData::<F, D>::from_bytes(
                deserialized.common_data_bytes,
                &gate_serializer,
            )
            .map_err(|_| anyhow::Error::msg("CommonCircuitData deserialization failed."))?;

            let mut buffer: Vec<u8> = vec![];
            proof.read_to_end(&mut buffer)?;
            let recursive_proof: ProofWithPublicInputs<F, C, D> =
                ProofWithPublicInputs::from_bytes(buffer, &common_data).map_err(|_| {
                    anyhow::Error::msg("ProofWithPublicInputs deserialization failed.")
                })?;

            let stark = S::default();
            let circuit_config = CircuitConfig::standard_recursion_config();
            let recursive_circuit = recursive_mozak_stark_circuit::<F, C, D>(
                &stark,
                &deserialized.degree_bits,
                &circuit_config,
                &config,
                12,
            );
            recursive_circuit.circuit.verify(recursive_proof)?;

            println!("Recursive proof verified successfully!");
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
            let trace = generate_memory_init_trace(&program);
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

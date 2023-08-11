#![deny(clippy::pedantic)]
#![deny(clippy::cargo)]
use std::io::{Read, Write};

use anyhow::Result;
use clap::{Parser, Subcommand};
use clio::{Input, Output};
use log::debug;
use mozak_circuits::stark::mozak_stark::MozakStark;
use mozak_circuits::stark::proof::AllProof;
use mozak_circuits::stark::prover::prove;
use mozak_circuits::stark::verifier::verify_proof;
use mozak_circuits::test_utils::{standard_faster_config, ProveAndVerify, C, D, F, S};
use mozak_vm::elf::Program;
use mozak_vm::state::State;
use mozak_vm::vm::step;
use plonky2::util::timing::TimingTree;
use shadow_rs::shadow;

shadow!(build);

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
    /// Show build info available to shadow_rs
    BuildInfo,
    /// Decode a given ELF and prints the program
    Decode { elf: Input },
    /// Decode and execute a given ELF. Prints the final state of
    /// the registers
    Run { elf: Input },
    /// Prove and verify the execution of a given ELF
    ProveAndVerify { elf: Input },
    /// Prove the execution of given ELF and write proof to file.
    Prove { elf: Input, proof: Output },
    /// Verify the given proof from file.
    Verify { proof: Input },
}
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
#[clap(action=ArgAction::SetFalse)]
struct Args {
    #[clap(long, short, action)]
    debug: bool,
}

fn build_info() {
    println!("debug:{}", shadow_rs::is_debug()); // check if this is a debug build. e.g 'true/false'
    println!("branch:{}", shadow_rs::branch()); // get current project branch. e.g 'master/develop'
    println!("tag:{}", shadow_rs::tag()); // get current project tag. e.g 'v1.3.5'
    println!("git_clean:{}", shadow_rs::git_clean()); // get current project clean. e.g 'true/false'
    println!("git_status_file:{}", shadow_rs::git_status_file()); // get current project statue file. e.g '  * examples/builtin_fn.rs (dirty)'

    println!("{}", build::VERSION);
    println!("{}", build::CLAP_LONG_VERSION);
    println!("{}", build::BRANCH);
    println!("{}", build::COMMIT_HASH);
    println!("{}", build::COMMIT_DATE);
    println!("{}", build::COMMIT_AUTHOR);
    println!("{}", build::COMMIT_EMAIL);

    println!("{}", build::BUILD_OS);
    println!("{}", build::RUST_VERSION);
    println!("{}", build::RUST_CHANNEL);
    println!("{}", build::CARGO_VERSION);
    println!("{}", build::PKG_VERSION);
    println!("{}", build::CARGO_TREE);
    println!("{}", build::CARGO_MANIFEST_DIR);

    println!("{}", build::PROJECT_NAME);
    println!("{}", build::BUILD_TIME);
    println!("{}", build::BUILD_RUST_CHANNEL);
    println!("{}", build::GIT_CLEAN);
    println!("{}", build::GIT_STATUS_FILE);
}

fn load_program(mut elf: Input) -> Result<Program> {
    let mut elf_bytes = Vec::new();
    let bytes_read = elf.read_to_end(&mut elf_bytes)?;
    debug!("Read {bytes_read} of ELF data.");
    Program::load_elf(&elf_bytes)
}

/// Run me eg like `cargo run -- -vvv run vm/tests/testdata/rv32ui-p-addi`
fn main() -> Result<()> {
    let cli = Cli::parse();
    let args = Args::parse();
    env_logger::Builder::new()
        .filter_level(cli.verbose.log_level_filter())
        .init();
    if let Command::BuildInfo = cli.command {
        build_info();
    } else {
        match cli.command {
            Command::Decode { elf } => {
                let program = load_program(elf)?;
                debug!("{program:?}");
            }
            Command::Run { elf } => {
                let program = load_program(elf)?;
                let state = State::from(&program);
                let state = step(&program, state)?.last_state;
                debug!("{:?}", state.registers);
            }
            Command::ProveAndVerify { elf } => {
                let program = load_program(elf)?;
                let state = State::from(&program);
                let record = step(&program, state)?;
                MozakStark::prove_and_verify(&program, &record.executed)?;
            }
            Command::Prove { elf, mut proof } => {
                let program = load_program(elf)?;
                let state = State::from(&program);
                let record = step(&program, state)?;
                let stark = if args.debug {
                    S::default()
                } else {
                    MozakStark::default_debug()
                };
                let config = standard_faster_config();

                let all_proof = prove::<F, C, D>(
                    &program,
                    &record.executed,
                    &stark,
                    &config,
                    &mut TimingTree::default(),
                )?;
                let s = all_proof.serialize_proof_to_flexbuffer()?;
                proof.write_all(s.view())?;
                debug!("proof generated successfully!");
            }
            Command::Verify { mut proof } => {
                let stark = S::default();
                let config = standard_faster_config();

                let mut buffer: Vec<u8> = vec![];
                proof.read_to_end(&mut buffer)?;
                let all_proof = AllProof::<F, C, D>::deserialize_proof_from_flexbuffer(&buffer)?;
                verify_proof(stark, all_proof, &config)?;
                debug!("proof verified successfully!");
            }
            Command::BuildInfo => unreachable!(),
        }
    }
    Ok(())
}

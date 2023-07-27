#![deny(clippy::pedantic)]
#![deny(clippy::cargo)]
use std::io::Read;

use clap::{Parser, ValueEnum};
use clio::Input;
use log::debug;
use mozak_circuits::stark::mozak_stark::MozakStark;
use mozak_circuits::test_utils::ProveAndVerify;
use mozak_vm::elf::Program;
use mozak_vm::state::State;
use mozak_vm::vm::step;
use shadow_rs::shadow;

shadow!(build);

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[clap(flatten)]
    verbose: clap_verbosity_flag::Verbosity,
    #[arg(value_enum)]
    command: Command,
    #[clap(value_parser, default_value = "-")]
    elf: Input,
}

#[derive(Copy, Clone, Debug, Parser, PartialEq, ValueEnum)]
enum Command {
    /// Show build info available to shadow_rs
    BuildInfo,
    /// Decode a given ELF and prints the program
    Decode,
    /// Decode and execute a given ELF. Prints the final state of
    /// the registers
    Run,
    /// Prove and verify the execution of a given ELF
    Prove,
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

/// Run me eg like `cargo run -- -vvv run vm/tests/testdata/rv32ui-p-addi`
fn main() -> anyhow::Result<()> {
    let mut cli = Cli::parse();
    env_logger::Builder::new()
        .filter_level(cli.verbose.log_level_filter())
        .init();
    if let Command::BuildInfo = cli.command {
        build_info();
    } else {
        let mut elf_bytes = Vec::new();
        let bytes_read = cli.elf.read_to_end(&mut elf_bytes)?;
        debug!("Read {bytes_read} of ELF data.");

        match cli.command {
            Command::Decode => {
                let program = Program::load_elf(&elf_bytes)?;
                debug!("{program:?}");
            }
            Command::Run => {
                let program = Program::load_elf(&elf_bytes)?;
                let state = State::from(program);
                let state = step(state)?.last_state;
                debug!("{:?}", state.registers);
            }
            Command::Prove => {
                let program = Program::load_elf(&elf_bytes)?;
                let state = State::from(program);
                let record = step(state)?;
                MozakStark::prove_and_verify(&record.executed)?;
            }
            Command::BuildInfo => unreachable!(),
        }
    }
    Ok(())
}

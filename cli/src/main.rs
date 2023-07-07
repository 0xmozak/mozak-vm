#![deny(clippy::pedantic)]
#![deny(clippy::cargo)]
use std::io::Read;

use clap::{Parser, ValueEnum};
use clio::Input;
use log::debug;
use mozak_circuits::test_utils::simple_proof_test;
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
    Decode,
    Run,
    Prove,
}

/// Run me eg like `cargo run -- -vvv run vm/tests/testdata/rv32ui-p-addi`
fn main() -> anyhow::Result<()> {
    let mut cli = Cli::parse();
    env_logger::Builder::new()
        .filter_level(cli.verbose.log_level_filter())
        .init();

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
            simple_proof_test(&record.executed)?;
        }
    }
    Ok(())
}

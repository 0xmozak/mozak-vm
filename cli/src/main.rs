#![deny(clippy::pedantic)]
#![deny(clippy::cargo)]
use std::io::Read;

use clap::{Parser, Subcommand};
use clio::Input;
use log::debug;
use mozak_circuits::test_utils::simple_proof_test;
use mozak_vm::elf::Program;
use mozak_vm::state::State;
use mozak_vm::vm::step;
use shadow_rs::shadow;

shadow!(build);

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[clap(value_parser, default_value = "-")]
    elf: Input,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Clone, Debug)]
enum Command {
    Decode,
    Run,
    Prove,
}

// /// Mozak VM: RISC-V ISA based zkVM
// #[derive(Parser, Debug)]
// #[command(author, version, about, long_about = None)]
// struct Args {
//     /// Name of the person to greet
//     #[arg(short, long)]
//     name: String,

//     /// Number of times to greet
//     #[arg(short, long, default_value_t = 1)]
//     count: u8,
// }

fn main() -> anyhow::Result<()> {
    env_logger::init();
    let mut cli = Cli::parse();
    let mut elf_bytes = Vec::new();
    let _x = cli.elf.read_to_end(&mut elf_bytes)?;
    match cli.command {
        Command::Decode => {
            let program = Program::load_elf(&elf_bytes)?;
            debug!("{program:?}");
        }
        Command::Run => {
            let program = Program::load_elf(&elf_bytes)?;
            debug!("{program:?}");
            let state = State::from(program);
            let state = step(state)?.last_state;
            let r = state.registers;
            debug!("{r:?}");
        }
        Command::Prove => {
            let program = Program::load_elf(&elf_bytes)?;
            debug!("{program:?}");
            let state = State::from(program);
            let record = step(state)?;
            let state = record.last_state;
            let r = state.registers;
            debug!("{r:?}");
            simple_proof_test(&record.executed)?;
        }
    }
    Ok(())
}

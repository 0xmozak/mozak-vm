use clap::Parser;
use clio::Input;
use mozak_circuits::test_utils::F;
use mozak_cli::runner::{load_program, raw_tapes_from_system_tape};
use mozak_runner::state::State;
use mozak_runner::vm::step;

#[derive(Parser, Debug, Clone)]
struct Cli {
    #[clap(flatten)]
    verbose: clap_verbosity_flag::Verbosity,
    elf: Input,
    #[arg(long)]
    system_tape: Option<Input>,
    #[arg(long)]
    self_prog_id: String,
}

fn main() {
    let args = Cli::parse();
    env_logger::Builder::new()
        .filter_level(args.verbose.log_level_filter())
        .init();
    let raw_tapes = raw_tapes_from_system_tape(args.system_tape, args.self_prog_id.into());
    let program = load_program(args.elf).unwrap();
    let state: State<F> = State::new(program.clone(), raw_tapes);
    step(&program, state).unwrap();
}

use clap::Parser;
use clio::Input;
use mozak_circuits::test_utils::{C, D, F};
use mozak_cli::runner::{get_self_prog_id, load_program, raw_tapes_from_system_tape};
use mozak_runner::state::State;
use mozak_runner::vm::step;
use starky::config::StarkConfig;

#[derive(Parser, Debug, Clone)]
struct Cli {
    #[clap(flatten)]
    verbose: clap_verbosity_flag::Verbosity,
    elf: Input,
    #[arg(long)]
    system_tape: Option<Input>,
}

fn main() {
    let args = Cli::parse();
    env_logger::Builder::new()
        .filter_level(args.verbose.log_level_filter())
        .init();
    let config = StarkConfig::standard_fast_config();
    let program = load_program(args.elf).unwrap();
    let self_prog_id = get_self_prog_id::<F, C, D>(&program, &config);

    let raw_tapes = raw_tapes_from_system_tape(args.system_tape, self_prog_id);

    let state: State<F> = State::new(program.clone(), raw_tapes);
    step(&program, state).unwrap();
}

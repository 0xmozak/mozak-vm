use clap::Parser;
use clio::Input;
use mozak_circuits::test_utils::{C, D, F};
use mozak_cli::runner::{get_self_prog_id, load_program};
use starky::config::StarkConfig;

#[derive(Parser, Debug, Clone)]
struct Cli {
    elf: Input,
}
fn main() {
    let args = Cli::parse();
    let config = StarkConfig::standard_fast_config();
    let program = load_program(args.elf).unwrap();
    let self_prog_id = get_self_prog_id::<F, C, D>(&program, &config);
    println!("{self_prog_id:?}");
}

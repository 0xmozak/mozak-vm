use mozak_runner::elf::{MozakRunTimeArguments, Program};
use mozak_runner::state::State;
use mozak_runner::vm::step;
use plonky2::field::goldilocks_field::GoldilocksField;

use crate::stark::mozak_stark::MozakStark;
use crate::test_utils::ProveAndVerify;

const FIBO_ELF_EXAMPLE_PATH: &str = "examples/target/riscv32im-mozak-zkvm-elf/release/fibonacci";

#[test]
fn test_fibonacci() {
    let elf_path = std::env::current_dir()
        .unwrap()
        .parent()
        .unwrap()
        .join(FIBO_ELF_EXAMPLE_PATH);
    let elf = std::fs::read(elf_path).expect(
        "Reading the fibonacci elf should not fail.
            You may need to build the fibonacci program within the examples directory
            eg. `cd examples/fibonacci && cargo build --release`",
    );
    let program =
        Program::load_program(&elf, &MozakRunTimeArguments::new(&[0; 32], &[], &[])).unwrap();
    let state = State::<GoldilocksField>::new(program.clone());
    let record = step(&program, state).unwrap();
    MozakStark::prove_and_verify(&program, &record).unwrap();
}

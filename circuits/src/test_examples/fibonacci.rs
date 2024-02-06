use mozak_runner::elf::{Program, RuntimeArguments};
use mozak_runner::state::State;
use mozak_runner::vm::step;
use plonky2::field::goldilocks_field::GoldilocksField;

use crate::stark::mozak_stark::MozakStark;
use crate::test_utils::ProveAndVerify;

#[test]
fn test_fibonacci() {
    let args = RuntimeArguments::default();
    let program = Program::mozak_load_program(mozak_examples::FIBONACCI_ELF, &args).unwrap();
    let state = State::<GoldilocksField>::new(program.clone(), args);
    let record = step(&program, state).unwrap();
    MozakStark::prove_and_verify(&program, &record).unwrap();
}

#[test]
fn test_fibonacci_mozak_elf() {
    let args = RuntimeArguments::default();
    let program = Program::mozak_load_program(mozak_examples::FIBONACCI_ELF, &args).unwrap();
    let state = State::<GoldilocksField>::new(program.clone(), args);
    let record = step(&program, state).unwrap();
    MozakStark::prove_and_verify(&program, &record).unwrap();
}
#[test]
fn test_fibonacci_mozak_elf_new_api() {
    let args = RuntimeArguments::default();
    let program =
        Program::mozak_load_program(mozak_examples::FIBONACCI_INPUT_ELF_NEW_API, &args).unwrap();
    let state = State::<GoldilocksField>::new(program.clone(), args);
    let record = step(&program, state).unwrap();
    MozakStark::prove_and_verify(&program, &record).unwrap();
}

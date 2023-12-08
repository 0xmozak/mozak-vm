use mozak_runner::elf::{Program, RuntimeArguments};
use mozak_runner::state::State;
use mozak_runner::vm::step;
use plonky2::field::goldilocks_field::GoldilocksField;

use crate::stark::mozak_stark::MozakStark;
use crate::test_utils::ProveAndVerify;

#[test]
fn test_fibonacci() {
    let program = Program::load_program(
        mozak_examples::FIBONACCI_ELF,
        &RuntimeArguments::new(&[0; 32], 0.0, &[], &[]),
    )
    .unwrap();
    let state = State::<GoldilocksField>::new(program.clone());
    let record = step(&program, state).unwrap();
    MozakStark::prove_and_verify(&program, &record).unwrap();
}

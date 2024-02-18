use mozak_runner::elf::{Program, RuntimeArguments};
use mozak_runner::state::State;
use mozak_runner::vm::step;
use plonky2::field::goldilocks_field::GoldilocksField;

use crate::stark::mozak_stark::MozakStark;
use crate::test_utils::ProveAndVerify;

#[test]
fn test_fibonacci() {
    let program = Program::load_elf(mozak_examples::FIBONACCI_ELF).unwrap();
    let state = State::<GoldilocksField>::new(program.clone(), RuntimeArguments::default());
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
// Allow this because assert string from `prove_and_verify` is not constant ...
#[allow(clippy::should_panic_without_expect)]
#[should_panic]
// NOTE: this test should panic since ELF is expecting to have non-empty input
fn test_fibonacci_mozak_elf_new_api_empty_args() {
    let args = RuntimeArguments::default();
    let program =
        Program::mozak_load_program(mozak_examples::FIBONACCI_INPUT_ELF_NEW_API, &args).unwrap();
    let state = State::<GoldilocksField>::new(program.clone(), args);
    let record = step(&program, state).unwrap();
    MozakStark::prove_and_verify(&program, &record).unwrap();
}
#[test]
fn test_fibonacci_mozak_elf_new_api() {
    let fibonacci = |n: u32| -> u32 {
        if n < 2 {
            return n;
        }
        let (mut curr, mut last) = (1_u32, 0_u32);
        for _i in 0..(n - 2) {
            (curr, last) = (curr.wrapping_add(last), curr);
        }
        curr
    };
    let n: u32 = 16;
    let out = fibonacci(n);
    let args = RuntimeArguments::new(
        vec![],
        n.to_le_bytes().to_vec(),
        out.to_le_bytes().to_vec(),
        vec![],
    );
    let program =
        Program::mozak_load_program(mozak_examples::FIBONACCI_INPUT_ELF_NEW_API, &args).unwrap();
    let state = State::<GoldilocksField>::new(program.clone(), args);
    let record = step(&program, state).unwrap();
    MozakStark::prove_and_verify(&program, &record).unwrap();
}

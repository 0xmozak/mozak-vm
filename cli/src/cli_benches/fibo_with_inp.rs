use mozak_circuits::test_utils::prove_and_verify_mozak_stark;
use mozak_runner::elf::{Program, RuntimeArguments};
use mozak_runner::state::State;
use mozak_runner::vm::step;
use plonky2::field::goldilocks_field::GoldilocksField;
use starky::config::StarkConfig;

fn fibonacci(n: u32) -> u32 {
    if n < 2 {
        return n;
    }
    let (mut curr, mut last) = (1_u32, 0_u32);
    for _i in 0..(n - 2) {
        (curr, last) = (curr.wrapping_add(last), curr);
    }
    curr
}

pub fn fibonacci_with_input(n: u32) -> Result<(), anyhow::Error> {
    let out = fibonacci(n);
    let program = Program::load_program(
        mozak_examples::FIBONACCI_INPUT_ELF,
        &RuntimeArguments::new(&[0; 32], 0.0, &n.to_le_bytes(), &out.to_le_bytes()),
    )
    .unwrap();
    let state = State::<GoldilocksField>::new(program.clone());
    let record = step(&program, state).unwrap();
    prove_and_verify_mozak_stark(&program, &record, &StarkConfig::standard_fast_config())
}

#[cfg(test)]
mod tests {
    use crate::cli_benches::benches::{BenchArgs, BenchFunction};

    #[test]
    fn test_fibonacci_with_input() {
        let n = 10;
        super::fibonacci_with_input(n).unwrap();
    }

    #[test]
    fn test_fibonacci_with_input_run() {
        let n = 10;
        let bench = BenchArgs {
            function: BenchFunction::FiboInputBench { n },
        };
        bench.run().unwrap();
    }
}

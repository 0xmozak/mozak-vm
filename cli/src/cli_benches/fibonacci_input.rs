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

pub fn fibonacci_input(n: u32) -> Result<(), anyhow::Error> {
    let program = Program::load_elf(mozak_examples::FIBONACCI_INPUT_ELF).unwrap();
    let out = fibonacci(n);
    let state = State::<GoldilocksField>::new(program.clone(), RuntimeArguments {
        self_prog_id: vec![],
        cast_list: vec![],
        io_tape_private: n.to_le_bytes().to_vec(),
        io_tape_public: out.to_le_bytes().to_vec(),
        call_tape: vec![],
        event_tape: vec![],
    });
    let record = step(&program, state).unwrap();
    prove_and_verify_mozak_stark(&program, &record, &StarkConfig::standard_fast_config())
}

pub fn fibonacci_input_mozak_elf(n: u32) -> Result<(), anyhow::Error> {
    let out = fibonacci(n);
    let args = RuntimeArguments::new(
        vec![],
        vec![],
        n.to_le_bytes().to_vec(),
        out.to_le_bytes().to_vec(),
        vec![],
        vec![],
    );
    let program = Program::mozak_load_program(mozak_examples::FIBONACCI_INPUT_ELF, &args).unwrap();
    let state = State::<GoldilocksField>::new(program.clone(), args);
    let record = step(&program, state).unwrap();
    prove_and_verify_mozak_stark(&program, &record, &StarkConfig::standard_fast_config())
}

#[cfg(test)]
mod tests {
    use crate::cli_benches::benches::{BenchArgs, BenchFunction};

    #[test]
    fn test_fibonacci_with_input() {
        let n = 10;
        super::fibonacci_input(n).unwrap();
    }

    #[test]
    fn test_fibonacci_with_input_mozak_elf() {
        let n = 10;
        super::fibonacci_input_mozak_elf(n).unwrap();
    }

    #[test]
    fn test_fibonacci_with_input_run() {
        let n = 10;
        let bench = BenchArgs {
            function: BenchFunction::FiboInputBench { n },
        };
        bench.run().unwrap();
    }

    #[test]
    fn test_fibonacci_with_input_mozak_elf_run() {
        let n = 10;
        let bench = BenchArgs {
            function: BenchFunction::FiboInputBenchMozakElf { n },
        };
        bench.run().unwrap();
    }
}

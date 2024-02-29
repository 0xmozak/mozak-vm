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
    let program = Program::vanilla_load_elf(mozak_examples::FIBONACCI_INPUT_ELF).unwrap();
    let out = fibonacci(n);
    // Note: first thing to notice that FIBO_INPUT_ELF is actually mozak-ELF, but it
    // is OK to use `vanilla` loader. Vanilla loader will still work, because
    // mozak-ELF is still the valid ELF. Second things related to the fact that
    // `State::new` API indeed accepts `args` but internally it assumes to take case
    // of external arguments thru mozak-ro-memory, and since vanilla loader was
    // used, there is not mozak-ro-memory inside Program. Third thing is related to
    // how rust-fibo ELF reads and writes io-tapes - it will just get zeros inside
    // its internal buffers, so fibo-code will see input = 0, so output will be also
    // 0, which is OK. In the next PRs we will remove old-io-tapes, and so also will
    // remove this legacy_ecall_api ...
    let state = State::<GoldilocksField>::legacy_ecall_api_new(program.clone(), RuntimeArguments {
        io_tape_private: n.to_le_bytes().to_vec(),
        io_tape_public: out.to_le_bytes().to_vec(),
        ..Default::default()
    });
    let record = step(&program, state).unwrap();
    prove_and_verify_mozak_stark(&program, &record, &StarkConfig::standard_fast_config())
}

pub fn fibonacci_input_mozak_elf(n: u32) -> Result<(), anyhow::Error> {
    let out = fibonacci(n);
    let args = RuntimeArguments {
        io_tape_private: n.to_le_bytes().to_vec(),
        io_tape_public: out.to_le_bytes().to_vec(),
        ..Default::default()
    };
    let program = Program::mozak_load_program(mozak_examples::FIBONACCI_INPUT_ELF, &args).unwrap();
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

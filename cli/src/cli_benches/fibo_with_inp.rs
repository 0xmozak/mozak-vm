use mozak_circuits::stark::mozak_stark::MozakStark;
use mozak_circuits::test_utils::ProveAndVerify;
use mozak_runner::elf::Program;
use mozak_runner::state::State;
use mozak_runner::vm::step;
use plonky2::field::goldilocks_field::GoldilocksField;

const FIBO_INP_ELF_EXAMPLE_PATH: &str =
    "examples/target/riscv32im-mozak-zkvm-elf/release/fibonacci-input";

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
    let elf_path = std::env::current_dir()
        .unwrap()
        .parent()
        .unwrap()
        .join(FIBO_INP_ELF_EXAMPLE_PATH);
    let elf = std::fs::read(elf_path).expect(
        "Reading the fibonacci-input elf should not fail.
            You may need to build the fibonacci program within the examples directory
            eg. `cd examples/fibonacci-input && cargo build --release`",
    );
    let program = Program::load_elf(&elf).unwrap();
    let out = fibonacci(n);
    let state =
        State::<GoldilocksField>::new(program.clone(), &n.to_le_bytes(), &out.to_le_bytes());
    let record = step(&program, state).unwrap();
    MozakStark::prove_and_verify(&program, &record)
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

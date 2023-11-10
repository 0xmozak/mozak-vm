use anyhow::Result;
use clap::{Args as Args_, Subcommand};
use mozak_runner::elf::Program;
use mozak_runner::instruction::{Args, Instruction, Op};
use mozak_runner::state::State;
use mozak_runner::test_utils::simple_test_code;
use mozak_runner::vm::step;
use plonky2::field::goldilocks_field::GoldilocksField;

use crate::stark::mozak_stark::MozakStark;
use crate::test_utils::ProveAndVerify;

const FIBO_INP_ELF_EXAMPLE_PATH: &str =
    "examples/target/riscv32im-mozak-zkvm-elf/release/fibonacci-input";

pub fn sample_bench(reg_value: u32) -> Result<(), anyhow::Error> {
    let instructions = &[
        Instruction {
            op: Op::ADD,
            args: Args {
                rd: 1,
                rs1: 1,
                imm: 1_u32.wrapping_neg(),
                ..Args::default()
            },
        },
        Instruction {
            op: Op::BLT,
            args: Args {
                rs1: 0,
                rs2: 1,
                imm: 0,
                ..Args::default()
            },
        },
    ];
    let (program, record) = simple_test_code(instructions, &[], &[(1, reg_value)]);
    MozakStark::prove_and_verify(&program, &record)
}

pub fn fibonacci_with_input(n: u32, out: u32) -> Result<(), anyhow::Error> {
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
    let state =
        State::<GoldilocksField>::new(program.clone(), &n.to_le_bytes(), &out.to_le_bytes());
    let record = step(&program, state).unwrap();
    MozakStark::prove_and_verify(&program, &record)
}

#[derive(Debug, Args_, Clone)]
#[command(args_conflicts_with_subcommands = true)]
pub struct BenchArgs {
    #[command(subcommand)]
    pub function: BenchFunction,
}

#[derive(PartialEq, Debug, Subcommand, Clone)]
pub enum BenchFunction {
    SampleBench { iterations: u32 },
    FiboInputBench { n: u32, out: u32 },
}

impl BenchArgs {
    pub fn run(&self) -> Result<(), anyhow::Error> {
        match self.function {
            BenchFunction::SampleBench { iterations } => sample_bench(iterations),
            BenchFunction::FiboInputBench { n, out } => fibonacci_with_input(n, out),
        }
    }
}

/// Mostly intended just to debug the bench functions
#[cfg(test)]
mod tests {

    #[test]
    fn test_sample_bench() { super::sample_bench(123).unwrap(); }

    #[test]
    fn test_fibonacci_with_input() {
        let n = 10;
        let out = fibonacci(n).1;
        super::fibonacci_with_input(n, out).unwrap();
    }

    fn fibonacci(n: u32) -> (u32, u32) {
        if n < 2 {
            return (0, n);
        }
        let (mut curr, mut last) = (1_u64, 0_u64);
        for _i in 0..(n - 2) {
            (curr, last) = (curr + last, curr);
        }
        (
            (curr >> 32) as u32,
            u32::try_from(curr).expect("please try <= 40 input only for now"),
        )
    }

    #[test]
    fn test_sample_bench_run() {
        let bench = super::BenchArgs {
            function: super::BenchFunction::SampleBench { iterations: 123 },
        };
        bench.run().unwrap();
    }

    #[test]
    fn test_fibonacci_with_input_run() {
        let n = 10;
        let out = fibonacci(n).1;
        let bench = super::BenchArgs {
            function: super::BenchFunction::FiboInputBench { n, out },
        };
        bench.run().unwrap();
    }
}

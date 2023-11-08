use anyhow::Result;
use clap::{Args as Args_, Subcommand};
use mozak_runner::instruction::{Args, Instruction, Op};
use mozak_runner::test_utils::simple_test_code;

use crate::stark::mozak_stark::MozakStark;
use crate::test_utils::ProveAndVerify;

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

#[derive(Debug, Args_, Clone)]
#[command(args_conflicts_with_subcommands = true)]
pub struct BenchArgs {
    #[command(subcommand)]
    pub function: BenchFunction,
}

#[derive(PartialEq, Debug, Subcommand, Clone)]
pub enum BenchFunction {
    SampleBench { iterations: u32 },
}

impl BenchArgs {
    pub fn run(&self) -> Result<(), anyhow::Error> {
        match self.function {
            BenchFunction::SampleBench { iterations } => sample_bench(iterations),
        }
    }
}

/// Mostly intended just to debug the bench functions
mod tests {
    #[test]
    fn test_sample_bench() { super::sample_bench(123).unwrap(); }

    #[test]
    fn test_run() {
        let bench = super::BenchArgs {
            function: super::BenchFunction::SampleBench { iterations: 123 },
        };
        bench.run().unwrap();
    }
}

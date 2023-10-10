use anyhow::Result;
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

/// Currently we support only the bench
/// functions that take one argument
#[derive(PartialEq, Debug)]
pub enum BenchFunction {
    SampleBench,
}

impl BenchFunction {
    pub fn run(&self, parameter: u32) -> Result<(), anyhow::Error> {
        match self {
            BenchFunction::SampleBench => sample_bench(parameter),
        }
    }

    pub fn from_name(function_name: &str) -> Result<Self> {
        match function_name {
            "sample_bench" => Ok(BenchFunction::SampleBench),
            _ => Err(anyhow::anyhow!("Invalid bench function")),
        }
    }
}

/// Mostly intended just to debug the bench functions
mod tests {
    #[test]
    fn test_sample_bench() { super::sample_bench(123).unwrap(); }

    #[test]
    fn test_from_string() {
        assert_eq!(
            super::BenchFunction::from_name("sample_bench").unwrap(),
            super::BenchFunction::SampleBench
        );
    }

    #[test]
    fn test_run() {
        let function = super::BenchFunction::SampleBench;
        assert!(function.run(123).is_ok());
    }
}

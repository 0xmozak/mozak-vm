use clap::{Args as Args_, Subcommand};

use super::nop::nop_bench;
use super::poseidon2::poseidon2_bench;
use super::xor::xor_bench;

#[derive(Debug, Args_, Clone)]
#[command(args_conflicts_with_subcommands = true)]
pub struct BenchArgs {
    #[command(subcommand)]
    pub function: BenchFunction,
}

#[derive(PartialEq, Debug, Subcommand, Clone)]
pub enum BenchFunction {
    XorBench { iterations: u32 },
    NopBench { iterations: u32 },
    Poseidon2Bench { input_len: u32 },
}

impl BenchArgs {
    pub fn run(&self) -> Result<(), anyhow::Error> {
        match self.function {
            BenchFunction::XorBench { iterations } => xor_bench(iterations),
            BenchFunction::NopBench { iterations } => nop_bench(iterations),
            BenchFunction::Poseidon2Bench { input_len } => poseidon2_bench(input_len),
        }
    }
}

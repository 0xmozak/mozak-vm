use clap::{Args as Args_, Subcommand};

use super::fibo_with_inp::fibonacci_with_input;
use super::sample::sample_bench;
use super::xorstark2::xor_stark_plonky2;
use super::xorstark3::xor_stark_plonky3;

#[derive(Debug, Args_, Clone)]
#[command(args_conflicts_with_subcommands = true)]
pub struct BenchArgs {
    #[command(subcommand)]
    pub function: BenchFunction,
}

#[derive(PartialEq, Debug, Subcommand, Clone)]
pub enum BenchFunction {
    SampleBench { iterations: u32 },
    FiboInputBench { n: u32 },
    XorStark2 { n: u32 },
    XorStark3 { n: u32 },
}

impl BenchArgs {
    pub fn run(&self) -> Result<(), anyhow::Error> {
        match self.function {
            BenchFunction::SampleBench { iterations } => sample_bench(iterations),
            BenchFunction::FiboInputBench { n } => fibonacci_with_input(n),
            BenchFunction::XorStark2 { n } => xor_stark_plonky2(n),
            BenchFunction::XorStark3 { n } => xor_stark_plonky3(n),
        }
    }
}

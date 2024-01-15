use clap::{Args as Args_, Subcommand};

use super::fibonacci_input::{fibonacci_input, fibonacci_input_mozak_elf};
use super::nop::nop_bench;
use super::poseidon2::poseidon2_bench;
use super::sample::sample_bench;
use super::xor::xor_bench;

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
    FiboInputBenchMozakElf { n: u32 },
    XorBench { iterations: u32 },
    NopBench { iterations: u32 },
    Poseidon2Bench { input_len: u32 },
}

impl BenchArgs {
    pub fn run(&self) -> Result<(), anyhow::Error> {
        match self.function {
            BenchFunction::SampleBench { iterations } => sample_bench(iterations),
            BenchFunction::FiboInputBench { n } => fibonacci_input(n),
            BenchFunction::FiboInputBenchMozakElf { n } => fibonacci_input_mozak_elf(n),
            BenchFunction::XorBench { iterations } => xor_bench(iterations),
            BenchFunction::NopBench { iterations } => nop_bench(iterations),
            BenchFunction::Poseidon2Bench { input_len } => poseidon2_bench(input_len),
        }
    }
}

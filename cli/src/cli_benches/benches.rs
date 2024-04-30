use std::time::Duration;

use anyhow::Result;
use clap::{Args as Args_, Subcommand};

use super::nop::NopBench;
use super::omni::OmniBench;
use super::poseidon2::Poseidon2Bench;
use super::sort::{SortBench, SortBenchRecursive};
use super::xor::XorBench;

#[derive(Debug, Args_, Clone)]
#[command(args_conflicts_with_subcommands = true)]
pub struct BenchArgs {
    #[command(subcommand)]
    pub function: BenchFunction,
}

pub trait Bench {
    type Args;
    type Prepared;

    fn prepare(&self, args: &Self::Args) -> Self::Prepared;
    fn execute(&self, prepared: Self::Prepared) -> Result<()>;

    fn bench(&self, args: &Self::Args) -> Result<Duration> {
        let prepared = self.prepare(args);
        let start = std::time::Instant::now();
        self.execute(prepared)?;
        Ok(start.elapsed())
    }
}

#[derive(PartialEq, Debug, Subcommand, Clone)]
pub enum BenchFunction {
    XorBench {
        iterations: u32,
    },
    NopBench {
        iterations: u32,
    },
    Poseidon2Bench {
        input_len: u32,
    },
    /// Benchmarks (almost) every instruction.
    OmniBench {
        iterations: u32,
    },
    SortBench {
        n: u32,
    },
    SortBenchRecursive {
        n: u32,
    },
}

impl BenchArgs {
    pub fn bench(&self) -> Result<Duration> {
        match &self.function {
            BenchFunction::XorBench { iterations } => {
                let xor_bench = XorBench;
                xor_bench.bench(iterations)
            }
            BenchFunction::NopBench { iterations } => {
                let nop_bench = NopBench;
                nop_bench.bench(iterations)
            }
            BenchFunction::OmniBench { iterations } => {
                let omni_bench = OmniBench;
                omni_bench.bench(iterations)
            }
            BenchFunction::Poseidon2Bench { input_len } => {
                let poseidon2_bench = Poseidon2Bench;
                poseidon2_bench.bench(input_len)
            }
            BenchFunction::SortBench { n } => {
                let sort_bench = SortBench;
                sort_bench.bench(n)
            }
            BenchFunction::SortBenchRecursive { n } => {
                let sort_bench_recursive = SortBenchRecursive;
                sort_bench_recursive.bench(n)
            }
        }
    }
}

use std::time::Duration;

use anyhow::Result;
use clap::{Args as Args_, Subcommand};

use super::nop::NopBench;
use super::omni::OmniBench;
use super::poseidon2::Poseidon2Bench;
use super::sort::{
    BatchStarksSortBench, BatchStarksSortBenchRecursive, SortBench, SortBenchRecursive,
};
use super::vector_alloc::VectorAllocBench;
use super::xor::XorBench;

#[derive(Debug, Args_, Clone)]
#[command(args_conflicts_with_subcommands = true)]
pub struct BenchArgs {
    #[command(subcommand)]
    pub function: BenchFunction,
}

pub(crate) trait Bench {
    type Args;
    type Prepared;

    /// method to be executed to prepare the benchmark
    fn prepare(&self, args: &Self::Args) -> Self::Prepared;

    /// actual benchmark function, whose execution time is
    /// to be measured
    fn execute(&self, prepared: Self::Prepared) -> Result<()>;

    /// benchmark the `execute` function implemented through the
    /// trait `Bench`
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
    BatchStarksSortBench {
        n: u32,
    },
    BatchStarksSortBenchRecursive {
        n: u32,
    },
    VectorAllocBench {
        n: u32,
    },
}

impl BenchArgs {
    pub fn bench(&self) -> Result<Duration> {
        match &self.function {
            BenchFunction::XorBench { iterations } => XorBench.bench(iterations),
            BenchFunction::NopBench { iterations } => NopBench.bench(iterations),
            BenchFunction::OmniBench { iterations } => OmniBench.bench(iterations),
            BenchFunction::Poseidon2Bench { input_len } => Poseidon2Bench.bench(input_len),
            BenchFunction::SortBench { n } => SortBench.bench(n),
            BenchFunction::SortBenchRecursive { n } => SortBenchRecursive.bench(n),
            BenchFunction::BatchStarksSortBench { n } => BatchStarksSortBench.bench(n),
            BenchFunction::BatchStarksSortBenchRecursive { n } =>
                BatchStarksSortBenchRecursive.bench(n),
            BenchFunction::VectorAllocBench { n } => VectorAllocBench.bench(n),
        }
    }
}

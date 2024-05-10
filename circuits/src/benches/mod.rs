pub mod nop;
pub mod omni;
pub mod poseidon2;
pub mod sort;
pub mod xor;

use std::time::Duration;

use anyhow::Result;
pub use mozak_cli_args::bench_args::{BenchArgs, BenchFunction};
use nop::NopBench;
use omni::OmniBench;
use poseidon2::Poseidon2Bench;
use sort::{SortBench, SortBenchRecursive};
use xor::XorBench;

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

pub fn bench(args: &BenchArgs) -> Result<Duration> {
    match &args.function {
        BenchFunction::XorBench { iterations } => XorBench.bench(iterations),
        BenchFunction::NopBench { iterations } => NopBench.bench(iterations),
        BenchFunction::OmniBench { iterations } => OmniBench.bench(iterations),
        BenchFunction::Poseidon2Bench { input_len } => Poseidon2Bench.bench(input_len),
        BenchFunction::SortBench { n } => SortBench.bench(n),
        BenchFunction::SortBenchRecursive { n } => SortBenchRecursive.bench(n),
    }
}

use clap::{Args as Args_, Subcommand};

#[derive(Debug, Args_, Clone)]
#[command(args_conflicts_with_subcommands = true)]
pub struct BenchArgs {
    #[command(subcommand)]
    pub function: BenchFunction,
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

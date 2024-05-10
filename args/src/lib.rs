#![deny(clippy::pedantic)]
#![deny(clippy::cargo)]
#![allow(clippy::multiple_crate_versions)]
pub mod bench_args;

use bench_args::BenchArgs;
use clap::{Parser, Subcommand};
use clap_derive::Args;
use clio::{Input, Output};

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[clap(flatten)]
    pub verbose: clap_verbosity_flag::Verbosity,
    #[command(subcommand)]
    pub command: Command,
    /// Debug API, default is OFF, currently only `prove` command is supported
    #[arg(short, long)]
    pub debug: bool,
}

#[derive(Clone, Debug, Args)]
pub struct RunArgs {
    pub elf: Input,
    #[arg(long)]
    pub system_tape: Option<Input>,
    #[arg(long)]
    pub self_prog_id: Option<String>,
}

#[derive(Clone, Debug, Args)]
pub struct ProveArgs {
    pub elf: Input,
    pub proof: Output,
    #[arg(long)]
    pub system_tape: Option<Input>,
    #[arg(long)]
    pub self_prog_id: Option<String>,
    pub recursive_proof: Option<Output>,
}

#[derive(Clone, Debug, Subcommand)]
pub enum Command {
    /// Decode a given ELF and prints the program
    Decode { elf: Input },
    /// Decode and execute a given ELF. Prints the final state of
    /// the registers
    Run(RunArgs),
    /// Prove and verify the execution of a given ELF
    ProveAndVerify(RunArgs),
    /// Prove the execution of given ELF and write proof to file.
    Prove(ProveArgs),
    /// Verify the given proof from file.
    Verify { proof: Input },
    /// Verify the given recursive proof from file.
    VerifyRecursiveProof { proof: Input, verifier_key: Input },
    /// Builds a transaction bundle.
    BundleTransaction {
        /// System tape generated from native execution.
        #[arg(long, required = true)]
        system_tape: Input,
        /// Output file path of the serialized bundle.
        #[arg(long, default_value = "bundle")]
        bundle: Output,
    },
    /// Compute the Program Rom Hash of the given ELF.
    ProgramRomHash { elf: Input },
    /// Compute the Memory Init Hash of the given ELF.
    MemoryInitHash { elf: Input },
    /// Bench the function with given parameters
    Bench(BenchArgs),
}

#[allow(non_upper_case_globals)]
pub const parse: fn() -> Cli = Cli::parse;

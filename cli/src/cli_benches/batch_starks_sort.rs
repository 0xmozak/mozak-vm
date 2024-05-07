use anyhow::Result;
use mozak_circuits::test_utils::{F, prove_and_verify_batch_mozak_stark};
use mozak_runner::elf::Program;
use mozak_runner::vm::{ExecutionRecord};
use starky::config::StarkConfig;
use crate::cli_benches::sort::sort_prepare;

use super::benches::Bench;

pub fn batch_starks_sort_execute(result: Result<(Program, ExecutionRecord<F>)>) -> Result<()> {
    let (program, record) = result?;
    prove_and_verify_batch_mozak_stark(&program, &record, &StarkConfig::standard_fast_config())
}

pub(crate) struct BatchStarksSortBench;

impl Bench for BatchStarksSortBench {
    type Args = u32;
    type Prepared = Result<(Program, ExecutionRecord<F>)>;

    fn prepare(&self, args: &Self::Args) -> Self::Prepared { sort_prepare(*args) }

    fn execute(&self, prepared: Self::Prepared) -> Result<()> { batch_starks_sort_execute(prepared) }
}

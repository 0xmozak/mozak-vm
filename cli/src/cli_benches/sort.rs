use anyhow::Result;
use mozak_circuits::test_utils::{prove_and_verify_mozak_stark, F};
use mozak_examples::MOZAK_SORT_ELF;
use mozak_runner::elf::Program;
use mozak_runner::state::{RawTapes, State};
use mozak_runner::vm::{step, ExecutionRecord};
use starky::config::StarkConfig;

use super::benches::Bench;

pub fn sort_execute(result: Result<(Program, ExecutionRecord<F>)>) -> Result<()> {
    let (program, record) = result?;
    prove_and_verify_mozak_stark(&program, &record, &StarkConfig::standard_fast_config())
}
pub fn sort_prepare(n: u32) -> Result<(Program, ExecutionRecord<F>)> {
    let program = Program::vanilla_load_elf(MOZAK_SORT_ELF)?;
    let raw_tapes = RawTapes {
        public_tape: n.to_le_bytes().to_vec(),
        ..Default::default()
    };
    let state = State::new(program.clone(), raw_tapes);
    let record = step(&program, state)?;
    Ok((program, record))
}

pub(crate) struct SortBench;

impl Bench for SortBench {
    type Args = u32;
    type Prepared = Result<(Program, ExecutionRecord<F>)>;

    fn prepare(&self, args: &Self::Args) -> Self::Prepared { sort_prepare(*args) }

    fn execute(&self, prepared: Self::Prepared) -> Result<()> { sort_execute(prepared) }
}
#[cfg(test)]
mod tests {
    use anyhow::Result;

    use super::{sort_execute, sort_prepare};

    #[test]
    fn test_sort_bench() -> Result<()> {
        let n = 10;
        sort_execute(sort_prepare(n))
    }
}

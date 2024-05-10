use anyhow::Result;
use mozak_circuits::test_utils::{prove_and_verify_mozak_stark, F};
use mozak_examples::MOZAK_POSEIDON2_ELF;
use mozak_runner::elf::Program;
use mozak_runner::state::{RawTapes, State};
use mozak_runner::vm::{step, ExecutionRecord};
use starky::config::StarkConfig;

use super::benches::Bench;

pub fn poseidon2_elf_prepare(n: u32) -> Result<(Program, ExecutionRecord<F>)> {
    let program = Program::vanilla_load_elf(MOZAK_POSEIDON2_ELF)?;
    let raw_tapes = RawTapes {
        public_tape: n.to_le_bytes().to_vec(),
        ..Default::default()
    };
    let state = State::new(program.clone(), raw_tapes);
    let record = step(&program, state)?;
    Ok((program, record))
}
pub fn poseidon2_elf_execute(
    result: Result<(Program, ExecutionRecord<F>)>,
) -> Result<(), anyhow::Error> {
    let (program, record) = result?;
    prove_and_verify_mozak_stark(&program, &record, &StarkConfig::standard_fast_config())
}

pub(crate) struct Poseidon2ELFBench;

impl Bench for Poseidon2ELFBench {
    type Args = u32;
    type Prepared = Result<(Program, ExecutionRecord<F>)>;

    fn prepare(&self, args: &Self::Args) -> Self::Prepared { poseidon2_elf_prepare(*args) }

    fn execute(&self, prepared: Self::Prepared) -> anyhow::Result<()> {
        poseidon2_elf_execute(prepared)
    }
}

#[cfg(test)]
mod tests {
    use super::{poseidon2_elf_execute, poseidon2_elf_prepare};
    #[test]
    fn test_poseidon2_elf_with_run() -> anyhow::Result<()> {
        let n = 100;
        poseidon2_elf_execute(poseidon2_elf_prepare(n))
    }
}

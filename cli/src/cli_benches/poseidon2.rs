use mozak_circuits::test_utils::{
    create_poseidon2_test, prove_and_verify_mozak_stark, Poseidon2Test, F,
};
use mozak_runner::elf::Program;
use mozak_runner::vm::ExecutionRecord;
use starky::config::StarkConfig;

use super::benches::Bench;

pub fn poseidon2_execute(
    (program, record): (Program, ExecutionRecord<F>),
) -> Result<(), anyhow::Error> {
    prove_and_verify_mozak_stark(&program, &record, &StarkConfig::standard_fast_config())
}
pub fn poseidon2_prepare(input_len: u32) -> (Program, ExecutionRecord<F>) {
    let s: String = "dead_beef_feed_c0de".repeat(input_len as usize);
    create_poseidon2_test(&[Poseidon2Test {
        data: s,
        input_start_addr: 1024,
        output_start_addr: 1024 + input_len,
    }])
}

pub(crate) struct Poseidon2Bench;

impl Bench for Poseidon2Bench {
    type Args = u32;
    type Prepared = (Program, ExecutionRecord<F>);

    fn prepare(&self, args: &Self::Args) -> Self::Prepared { poseidon2_prepare(*args) }

    fn execute(&self, prepared: Self::Prepared) -> anyhow::Result<()> {
        poseidon2_execute(prepared)
    }
}

#[cfg(test)]
mod tests {
    use super::{poseidon2_execute, poseidon2_prepare};

    #[test]
    fn test_poseidon2_bench() -> anyhow::Result<()> {
        let input_len = 10;
        poseidon2_execute(poseidon2_prepare(input_len))
    }
}

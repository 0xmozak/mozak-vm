use mozak_runner::elf::Program;
use mozak_runner::vm::ExecutionRecord;
use starky::config::StarkConfig;

use super::Bench;
use crate::test_utils::{create_poseidon2_test, prove_and_verify_mozak_stark, Poseidon2Test, F};

pub(crate) struct Poseidon2Bench;

impl Bench for Poseidon2Bench {
    type Args = u32;
    type Prepared = (Program, ExecutionRecord<F>);

    fn prepare(&self, &input_len: &u32) -> (Program, ExecutionRecord<F>) {
        let s: String = "dead_beef_feed_c0de".repeat(input_len as usize);
        create_poseidon2_test(&[Poseidon2Test {
            data: s,
            input_start_addr: 1024,
            output_start_addr: 1024 + input_len,
        }])
    }

    fn execute(&self, (program, record): (Program, ExecutionRecord<F>)) -> anyhow::Result<()> {
        prove_and_verify_mozak_stark(&program, &record, &StarkConfig::standard_fast_config())
    }
}

#[cfg(test)]
mod tests {
    use super::Poseidon2Bench;
    use crate::benches::Bench;

    #[test]
    fn test_poseidon2_bench() -> anyhow::Result<()> {
        Poseidon2Bench {}.execute(Poseidon2Bench {}.prepare(&10))
    }
}

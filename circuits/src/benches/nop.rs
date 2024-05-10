use mozak_runner::code;
use mozak_runner::elf::Program;
use mozak_runner::instruction::{Args, Instruction, Op, NOP};
use mozak_runner::vm::ExecutionRecord;
use starky::config::StarkConfig;

use super::Bench;
use crate::test_utils::{prove_and_verify_mozak_stark, F};

pub(crate) struct NopBench;

impl Bench for NopBench {
    type Args = u32;
    type Prepared = (Program, ExecutionRecord<F>);

    fn prepare(&self, &iterations: &u32) -> Self::Prepared {
        let instructions = [
            Instruction {
                op: Op::ADD,
                args: Args {
                    rd: 1,
                    rs1: 1,
                    imm: 1_u32.wrapping_neg(),
                    ..Args::default()
                },
            },
            NOP,
            Instruction {
                op: Op::BLT,
                args: Args {
                    rs1: 0,
                    rs2: 1,
                    imm: 0,
                    ..Args::default()
                },
            },
        ];
        code::execute(instructions, &[], &[(1, iterations)])
    }

    fn execute(&self, (program, record): (Program, ExecutionRecord<F>)) -> anyhow::Result<()> {
        prove_and_verify_mozak_stark(&program, &record, &StarkConfig::standard_fast_config())
    }
}
#[cfg(test)]
mod tests {
    use super::NopBench;
    use crate::benches::Bench;

    #[test]
    fn test_nop_bench() -> anyhow::Result<()> { NopBench {}.execute(NopBench {}.prepare(&10)) }
}

use mozak_runner::code;
use mozak_runner::elf::Program;
use mozak_runner::instruction::{Args, Instruction, Op};
use mozak_runner::vm::ExecutionRecord;
use starky::config::StarkConfig;

use super::Bench;
use crate::test_utils::{prove_and_verify_mozak_stark, F};

pub(crate) struct XorBench;

impl Bench for XorBench {
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
            Instruction {
                op: Op::XOR,
                args: Args {
                    rd: 2,
                    rs1: 1,
                    imm: 0xDEAD_BEEF,
                    ..Args::default()
                },
            },
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
    use super::XorBench;
    use crate::benches::Bench;

    #[test]
    fn test_xor_bench() -> anyhow::Result<()> { XorBench {}.execute(XorBench {}.prepare(&10)) }
}

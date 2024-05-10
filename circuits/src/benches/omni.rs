use mozak_runner::code;
use mozak_runner::elf::Program;
use mozak_runner::instruction::{Args, Instruction, Op};
use mozak_runner::vm::ExecutionRecord;
use starky::config::StarkConfig;

use super::Bench;
use crate::test_utils::{prove_and_verify_mozak_stark, F};

pub(crate) struct OmniBench;

impl Bench for OmniBench {
    type Args = u32;
    type Prepared = (Program, ExecutionRecord<F>);

    /// Benchmark almost every instruction.
    ///
    /// Important: when extending, don't mess with register 1, because we need
    /// it as the loop variable.
    #[allow(clippy::too_many_lines)]
    fn prepare(&self, &iterations: &u32) -> Self::Prepared {
        let instructions = [
            Instruction {
                op: Op::ADD,
                args: Args {
                    rd: 2,
                    rs1: 1,
                    imm: 1_u32.wrapping_neg(),
                    ..Args::default()
                },
            },
            Instruction {
                op: Op::SUB,
                args: Args {
                    rd: 3,
                    rs1: 1,
                    imm: 1_u32.wrapping_neg(),
                    ..Args::default()
                },
            },
            Instruction {
                op: Op::XOR,
                args: Args {
                    rd: 3,
                    rs1: 1,
                    rs2: 2,
                    ..Args::default()
                },
            },
            Instruction {
                op: Op::OR,
                args: Args {
                    rd: 3,
                    rs1: 1,
                    rs2: 2,
                    ..Args::default()
                },
            },
            Instruction {
                op: Op::AND,
                args: Args {
                    rd: 3,
                    rs1: 1,
                    rs2: 2,
                    ..Args::default()
                },
            },
            Instruction {
                op: Op::SLL,
                args: Args {
                    rd: 4,
                    rs1: 1,
                    rs2: 2,
                    ..Args::default()
                },
            },
            Instruction {
                op: Op::SRL,
                args: Args {
                    rd: 4,
                    rs1: 1,
                    rs2: 2,
                    ..Args::default()
                },
            },
            Instruction {
                op: Op::SRA,
                args: Args {
                    rd: 4,
                    rs1: 1,
                    rs2: 2,
                    ..Args::default()
                },
            },
            Instruction {
                op: Op::SLT,
                args: Args {
                    rd: 4,
                    rs1: 1,
                    rs2: 2,
                    ..Args::default()
                },
            },
            Instruction {
                op: Op::SLTU,
                args: Args {
                    rd: 4,
                    rs1: 1,
                    rs2: 2,
                    ..Args::default()
                },
            },
            Instruction {
                op: Op::LB,
                args: Args {
                    rd: 4,
                    rs1: 1,
                    rs2: 2,
                    ..Args::default()
                },
            },
            Instruction {
                op: Op::LH,
                args: Args {
                    rd: 4,
                    rs1: 1,
                    rs2: 2,
                    ..Args::default()
                },
            },
            Instruction {
                op: Op::LW,
                args: Args {
                    rd: 4,
                    rs1: 1,
                    rs2: 2,
                    ..Args::default()
                },
            },
            Instruction {
                op: Op::LBU,
                args: Args {
                    rd: 4,
                    rs1: 1,
                    rs2: 2,
                    ..Args::default()
                },
            },
            Instruction {
                op: Op::SB,
                args: Args {
                    rd: 4,
                    rs1: 1,
                    rs2: 2,
                    ..Args::default()
                },
            },
            Instruction {
                op: Op::SH,
                args: Args {
                    rd: 4,
                    rs1: 1,
                    rs2: 2,
                    ..Args::default()
                },
            },
            Instruction {
                op: Op::SW,
                args: Args {
                    rd: 4,
                    rs1: 1,
                    rs2: 2,
                    ..Args::default()
                },
            },
            // TODO(Matthias): add branches, jumps and ecalls later.
            Instruction {
                op: Op::MUL,
                args: Args {
                    rd: 4,
                    rs1: 1,
                    rs2: 2,
                    ..Args::default()
                },
            },
            Instruction {
                op: Op::MULH,
                args: Args {
                    rd: 4,
                    rs1: 1,
                    rs2: 2,
                    ..Args::default()
                },
            },
            Instruction {
                op: Op::MULHU,
                args: Args {
                    rd: 4,
                    rs1: 1,
                    rs2: 2,
                    ..Args::default()
                },
            },
            Instruction {
                op: Op::MULHSU,
                args: Args {
                    rd: 4,
                    rs1: 1,
                    rs2: 2,
                    ..Args::default()
                },
            },
            Instruction {
                op: Op::DIV,
                args: Args {
                    rd: 4,
                    rs1: 1,
                    rs2: 2,
                    ..Args::default()
                },
            },
            Instruction {
                op: Op::DIVU,
                args: Args {
                    rd: 4,
                    rs1: 1,
                    rs2: 2,
                    ..Args::default()
                },
            },
            Instruction {
                op: Op::REM,
                args: Args {
                    rd: 4,
                    rs1: 1,
                    rs2: 2,
                    ..Args::default()
                },
            },
            Instruction {
                op: Op::REMU,
                args: Args {
                    rd: 4,
                    rs1: 1,
                    rs2: 2,
                    ..Args::default()
                },
            },
            // The instructions for the loop: count-down and a branch back to the start.
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
    use super::OmniBench;
    use crate::benches::Bench;

    #[test]
    fn test_omni_bench() -> anyhow::Result<()> { OmniBench {}.execute(OmniBench {}.prepare(&10)) }
}

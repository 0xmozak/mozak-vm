use itertools::chain;
use mozak_circuits::test_utils::prove_and_verify_mozak_stark;
use mozak_runner::code;
use mozak_runner::instruction::{Args, Instruction, Op};
use starky::config::StarkConfig;

#[allow(clippy::module_name_repetitions)]
pub fn omni_bench(iterations: u32) -> Result<(), anyhow::Error> {
    let instructions = chain![
        [
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
        ],
        [
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
        ]
    ]
    .collect::<Vec<_>>();
    let (program, record) = code::execute(instructions, &[], &[(1, iterations)]);
    prove_and_verify_mozak_stark(&program, &record, &StarkConfig::standard_fast_config())
}

#[cfg(test)]
mod tests {
    use crate::cli_benches::benches::{BenchArgs, BenchFunction};

    #[test]
    fn test_omni_bench() {
        let iterations = 10;
        super::omni_bench(iterations).unwrap();
    }

    #[test]
    fn test_omni_bench_with_run() {
        let iterations = 10;
        let bench = BenchArgs {
            function: BenchFunction::OmniBench { iterations },
        };
        bench.run().unwrap();
    }
}

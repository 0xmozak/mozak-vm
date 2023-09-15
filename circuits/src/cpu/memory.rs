#[cfg(test)]
#[allow(clippy::cast_possible_wrap)]
mod tests {
    use mozak_vm::instruction::{Args, Instruction, Op};
    use mozak_vm::test_utils::{simple_test_code, u32_extra, u8_extra};
    use proptest::prelude::ProptestConfig;
    use proptest::proptest;

    use crate::cpu::stark::CpuStark;
    use crate::stark::mozak_stark::MozakStark;
    use crate::test_utils::{ProveAndVerify, D, F};

    fn prove_sb<Stark: ProveAndVerify>(a: u32, b: u32) {
        let (program, record) = simple_test_code(
            &[Instruction {
                op: Op::SB,
                args: Args {
                    rs1: 6,
                    rs2: 7,
                    ..Args::default()
                },
            }],
            &[],
            &[],
            &[(6, a), (7, b)],
        );

        Stark::prove_and_verify(&program, &record).unwrap();
    }
    // NOTE: prove_lbu fails with MozakSnark
    fn prove_lbu<Stark: ProveAndVerify>(a: u32, b: u32) {
        let (program, record) = simple_test_code(
            &[Instruction {
                op: Op::LBU,
                args: Args {
                    rs1: 6,
                    rs2: 7,
                    ..Args::default()
                },
            }],
            &[],
            &[],
            &[(6, a), (7, b)],
        );

        Stark::prove_and_verify(&program, &record).unwrap();
    }

    fn prove_mem_read_write<Stark: ProveAndVerify>(offset: u32, imm: u32, content: u8) {
        let (program, record) = simple_test_code(
            &[
                Instruction {
                    op: Op::SB,
                    args: Args {
                        rs1: 1,
                        rs2: 2,
                        imm,
                        ..Args::default()
                    },
                },
                Instruction {
                    op: Op::LBU,
                    args: Args {
                        rs2: 2,
                        imm,
                        ..Args::default()
                    },
                },
            ],
            &[],
            &[],
            &[(1, content.into()), (2, offset)],
        );

        Stark::prove_and_verify(&program, &record).unwrap();
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(4))]
        #[test]
        fn prove_sb_cpu(a in u32_extra(), b in u32_extra()) {
            prove_sb::<CpuStark<F, D>>(a, b);
        }

        #[test]
        fn prove_lbu_cpu(a in u32_extra(), b in u32_extra()) {
            prove_lbu::<CpuStark<F, D>>(a, b);
        }

        #[test]
        fn prove_mem_read_write_cpu(offset in u32_extra(), imm in u32_extra(), content in u8_extra()) {
            prove_mem_read_write::<CpuStark<F, D>>(offset, imm, content);
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(1))]
        #[test]
        fn prove_sb_mozak(a in u32_extra(), b in u32_extra()) {
            prove_sb::<MozakStark<F, D>>(a, b);
        }

        #[test]
        fn prove_mem_read_write_mozak(offset in u32_extra(), imm in u32_extra(), content in u8_extra()) {
            prove_mem_read_write::<MozakStark<F, D>>(offset, imm, content);
        }
    }
}

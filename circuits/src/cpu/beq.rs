#[cfg(test)]
#[allow(clippy::cast_possible_wrap)]
mod tests {
    use mozak_vm::instruction::{Args, Instruction, Op};
    use mozak_vm::test_utils::{simple_test_code, state_before_final, u32_extra};
    use proptest::prelude::ProptestConfig;
    use proptest::proptest;

    use crate::cpu::stark::CpuStark;
    use crate::test_utils::ProveAndVerify;
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(4))]
        #[test]
        fn prove_beq_proptest(a in u32_extra(), b in u32_extra()) {
            let (program, record) = simple_test_code(
                &[
                    Instruction {
                        op: Op::BEQ,
                        args: Args {
                            rs1: 6,
                            rs2: 7,
                            branch_target: 8,
                            ..Args::default()
                        },
                    },
                    // if above branch is not taken R1 has value 10.
                    Instruction {
                        op: Op::ADD,
                        args: Args {
                            rd: 1,
                            imm: 10,
                            ..Args::default()
                        },
                    },
                ],
                &[],
                &[(6, a), (7, b)],
            );

            if a == b {
                assert_eq!(state_before_final(&record).get_register_value(1), 0);
            } else {
                assert_eq!(state_before_final(&record).get_register_value(1), 10);
            }

            CpuStark::prove_and_verify(&program, &record.executed).unwrap();
        }
        #[test]
        fn prove_bne_proptest(a in u32_extra(), b in u32_extra()) {
            let (program, record) = simple_test_code(
                &[
                    Instruction {
                        op: Op::BNE,
                        args: Args {
                            rs1: 6,
                            rs2: 7,
                            branch_target: 8,
                            ..Args::default()
                        },
                    },
                    // if above branch is not taken R1 has value 10.
                    Instruction {
                        op: Op::ADD,
                        args: Args {
                            rd: 1,
                            imm: 10,
                            ..Args::default()
                        },
                    },
                ],
                &[],
                &[(6, a), (7, b)],
            );
            if a == b {
                assert_eq!(state_before_final(&record).get_register_value(1), 10);
            } else {
                assert_eq!(state_before_final(&record).get_register_value(1), 0);
            }
            CpuStark::prove_and_verify(&program, &record.executed).unwrap();
        }
    }
}

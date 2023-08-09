use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use starky::constraint_consumer::ConstraintConsumer;

use super::columns::CpuState;

/// Constraints for BEQ and BNE.
pub(crate) fn constraints<P: PackedField>(
    lv: &CpuState<P>,
    nv: &CpuState<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    let bumped_pc = lv.inst.pc + P::Scalar::from_noncanonical_u64(4);
    let branched_pc = lv.inst.branch_target;
    // TODO: make diff a function on CpuState.
    let diff = lv.op1_value - lv.op2_value;

    // if `diff == 0`, then `is_equal != 0`.
    // We only need this intermediate variable to keep the constraint degree <= 3.
    let is_equal = lv.branch_equal;
    let diff_inv = lv.cmp_diff_inv;
    yield_constr.constraint(diff * diff_inv + is_equal - P::ONES);

    let next_pc = nv.inst.pc;
    yield_constr.constraint(lv.inst.ops.beq * is_equal * (next_pc - branched_pc));
    yield_constr.constraint(lv.inst.ops.beq * diff * (next_pc - bumped_pc));

    // For BNE branch happens when both operands are not equal so swap above
    // constraints.
    yield_constr.constraint(lv.inst.ops.bne * diff * (next_pc - branched_pc));
    yield_constr.constraint(lv.inst.ops.bne * is_equal * (next_pc - bumped_pc));
}

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

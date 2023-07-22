use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use starky::constraint_consumer::ConstraintConsumer;

use super::columns::CpuColumnsView;

/// Constraints for BEQ and BNE.
pub(crate) fn constraints<P: PackedField>(
    lv: &CpuColumnsView<P>,
    nv: &CpuColumnsView<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    let is_beq = lv.ops.beq;
    let is_bne = lv.ops.bne;
    let bumped_pc = lv.pc + P::Scalar::from_noncanonical_u64(4);
    let branched_pc = lv.imm_value;
    // TODO: make diff a function on CpuColumnsView.
    let diff = lv.op1_value - lv.op2_value;

    // if `diff == 0`, then `is_equal != 0`.
    // We only need this intermediate variable to keep the constraint degree <= 3.
    let is_equal = lv.branch_equal;
    let diff_inv = lv.branch_diff_inv;
    yield_constr.constraint(diff * diff_inv + is_equal - P::ONES);

    let next_pc = nv.pc;
    yield_constr.constraint(is_beq * is_equal * (next_pc - branched_pc));
    yield_constr.constraint(is_beq * diff * (next_pc - bumped_pc));

    // For BNE branch happens when both operands are not equal so swap above
    // constraints.
    yield_constr.constraint(is_bne * diff * (next_pc - branched_pc));
    yield_constr.constraint(is_bne * is_equal * (next_pc - bumped_pc));
}

#[cfg(test)]
#[allow(clippy::cast_possible_wrap)]
mod tests {
    use mozak_vm::instruction::{Args, Instruction, Op};
    use mozak_vm::test_utils::{simple_test_code, u32_extra};
    use proptest::prelude::ProptestConfig;
    use proptest::proptest;

    use crate::test_utils::simple_proof_test;
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(4))]
        #[test]
        fn prove_beq_proptest(a in u32_extra(), b in u32_extra()) {
            let record = simple_test_code(
                &[
                    Instruction {
                        op: Op::BEQ,
                        args: Args {
                            rd: 0,
                            rs1: 6,
                            rs2: 7,
                            imm: 8,
                        },
                    },
                    // if above branch is not taken R1 has value 10.
                    Instruction {
                        op: Op::ADD,
                        args: Args {
                            rd: 1,
                            rs1: 0,
                            rs2: 0,
                            imm: 10,
                        },
                    },
                ],
                &[],
                &[(6, a), (7, b)],
            );
            if a == b {
                assert_eq!(record.last_state.get_register_value(1), 0);
            } else {
                assert_eq!(record.last_state.get_register_value(1), 10);
            }
            simple_proof_test(&record.executed).unwrap();
        }
        #[test]
        fn prove_bne_proptest(a in u32_extra(), b in u32_extra()) {
            let record = simple_test_code(
                &[
                    Instruction {
                        op: Op::BNE,
                        args: Args {
                            rd: 0,
                            rs1: 6,
                            rs2: 7,
                            imm: 8,
                        },
                    },
                    // if above branch is not taken R1 has value 10.
                    Instruction {
                        op: Op::ADD,
                        args: Args {
                            rd: 1,
                            rs1: 0,
                            rs2: 0,
                            imm: 10,
                        },
                    },
                ],
                &[],
                &[(6, a), (7, b)],
            );
            if a == b {
                assert_eq!(record.last_state.get_register_value(1), 10);
            } else {
                assert_eq!(record.last_state.get_register_value(1), 0);
            }
            simple_proof_test(&record.executed).unwrap();
        }
    }
}

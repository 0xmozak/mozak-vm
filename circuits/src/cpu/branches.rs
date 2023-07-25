use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use starky::constraint_consumer::ConstraintConsumer;

use super::columns::CpuColumnsView;

/// Constraints for `less_than` and `ops_are_equal`
pub(crate) fn comparison_constraints<P: PackedField>(
    lv: &CpuColumnsView<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    let lt = lv.less_than;
    yield_constr.constraint(lt * (P::ONES - lt));

    let diff_fixed = lv.op1_full_range() - lv.op2_full_range();
    // TODO: range check
    let abs_diff = lv.cmp_abs_diff;

    // abs_diff calculation
    yield_constr.constraint((P::ONES - lt) * (abs_diff - diff_fixed));
    yield_constr.constraint(lt * (abs_diff + diff_fixed));

    // Force lt == 0, if op1 == op2:
    let diff = lv.op_diff();
    let diff_inv = lv.cmp_diff_inv;
    let ops_are_equal = lv.ops_are_equal;
    yield_constr.constraint(ops_are_equal * (ops_are_equal - P::ONES));
    yield_constr.constraint(diff * diff_inv + ops_are_equal - P::ONES);

    yield_constr.constraint(lt * ops_are_equal);
}

/// Constraints for conditional branch operations
pub(crate) fn constraints<P: PackedField>(
    lv: &CpuColumnsView<P>,
    nv: &CpuColumnsView<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    let is_blt = lv.ops.blt;
    let is_bltu = lv.ops.bltu;
    let is_bge = lv.ops.bge;
    let is_bgeu = lv.ops.bgeu;

    let bumped_pc = lv.pc + P::Scalar::from_noncanonical_u64(4);
    let branched_pc = lv.branch_target;
    let next_pc = nv.pc;

    let lt = lv.less_than;

    let diff = lv.op_diff();
    // if `diff == 0`, then `ops_are_equal != 0`.
    // We only need this intermediate variable to keep the constraint degree <= 3.
    let ops_are_equal = lv.ops_are_equal;

    yield_constr.constraint((is_blt + is_bltu) * lt * (next_pc - branched_pc));
    yield_constr.constraint((is_blt + is_bltu) * (P::ONES - lt) * (next_pc - bumped_pc));

    yield_constr.constraint((is_bge + is_bgeu) * lt * (next_pc - bumped_pc));
    yield_constr.constraint((is_bge + is_bgeu) * (P::ONES - lt) * (next_pc - branched_pc));

    yield_constr.constraint(lv.ops.beq * ops_are_equal * (next_pc - branched_pc));
    yield_constr.constraint(lv.ops.beq * diff * (next_pc - bumped_pc));

    yield_constr.constraint(lv.ops.bne * diff * (next_pc - branched_pc));
    yield_constr.constraint(lv.ops.bne * ops_are_equal * (next_pc - bumped_pc));
}

#[cfg(test)]
#[allow(clippy::cast_possible_wrap)]
mod tests {
    use mozak_vm::instruction::{Args, Instruction, Op};
    use mozak_vm::test_utils::{last_but_coda, simple_test_code, u32_extra};
    use proptest::prelude::ProptestConfig;
    use proptest::proptest;

    use crate::test_utils::simple_proof_test;
    fn test_cond_branch(a: u32, b: u32, op: Op) {
        assert!(matches!(op, Op::BLT | Op::BLTU | Op::BGE | Op::BGEU));
        let record = simple_test_code(
            &[
                Instruction {
                    op,
                    args: Args {
                        rd: 0,
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
        match op {
            Op::BLT =>
                if (a as i32) < (b as i32) {
                    assert_eq!(last_but_coda(&record).get_register_value(1), 0);
                } else {
                    assert_eq!(last_but_coda(&record).get_register_value(1), 10);
                },
            Op::BLTU =>
                if a < b {
                    assert_eq!(last_but_coda(&record).get_register_value(1), 0);
                } else {
                    assert_eq!(last_but_coda(&record).get_register_value(1), 10);
                },
            Op::BGE =>
                if (a as i32) >= (b as i32) {
                    assert_eq!(last_but_coda(&record).get_register_value(1), 0);
                } else {
                    assert_eq!(last_but_coda(&record).get_register_value(1), 10);
                },
            Op::BGEU =>
                if a >= b {
                    assert_eq!(last_but_coda(&record).get_register_value(1), 0);
                } else {
                    assert_eq!(last_but_coda(&record).get_register_value(1), 10);
                },
            _ => unreachable!(),
        }
        simple_proof_test(&record.executed).unwrap();
    }
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(4))]
        #[test]
        fn prove_blt_proptest(a in u32_extra(), b in u32_extra()) {
            test_cond_branch(a, b, Op::BLT);
        }
        #[test]
        fn prove_bltu_proptest(a in u32_extra(), b in u32_extra()) {
            test_cond_branch(a, b, Op::BLTU);
        }
        #[test]
        fn prove_bge_proptest(a in u32_extra(), b in u32_extra()) {
            test_cond_branch(a, b, Op::BGE);
        }
        #[test]
        fn prove_bgeu_proptest(a in u32_extra(), b in u32_extra()) {
            test_cond_branch(a, b, Op::BGEU);
        }

        #[test]
        fn prove_beq_proptest(a in u32_extra(), b in u32_extra()) {
            let record = simple_test_code(
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
                assert_eq!(last_but_coda(&record).get_register_value(1), 0);
            } else {
                assert_eq!(last_but_coda(&record).get_register_value(1), 10);
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
                assert_eq!(last_but_coda(&record).get_register_value(1), 10);
            } else {
                assert_eq!(last_but_coda(&record).get_register_value(1), 0);
            }
            simple_proof_test(&record.executed).unwrap();
        }
    }
}

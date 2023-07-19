use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use starky::constraint_consumer::ConstraintConsumer;

use super::columns::{
    BRANCH_DIFF_INV, COL_EQUAL, COL_IMM_VALUE, COL_OP1_VALUE, COL_OP2_VALUE, COL_PC, COL_S_BEQ,
    NUM_CPU_COLS,
};

pub(crate) fn constraints<P: PackedField>(
    lv: &[P; NUM_CPU_COLS],
    nv: &[P; NUM_CPU_COLS],
    yield_constr: &mut ConstraintConsumer<P>,
) {
    let is_beq = lv[COL_S_BEQ];

    let op1 = lv[COL_OP1_VALUE];
    let op2 = lv[COL_OP2_VALUE];

    let diff = op1 - op2;
    let diff_inv = lv[BRANCH_DIFF_INV];
    let branch = P::ONES - diff * diff_inv;
    let equal = lv[COL_EQUAL];
    let updated_pc = nv[COL_PC];

    // check equal is 0 or 1
    yield_constr.constraint(is_beq * equal * (P::ONES - equal));

    // if equal is 1 then diff should be 0
    yield_constr.constraint(is_beq * equal * diff);

    // equal should match with branch
    yield_constr.constraint(is_beq * (branch - equal));

    // check PC updated correctly
    let pc_next: P =
        (P::ONES - equal) * (updated_pc - (lv[COL_PC] + P::Scalar::from_noncanonical_u64(4)));
    let pc_branch: P = equal * (updated_pc - lv[COL_IMM_VALUE]);
    yield_constr.constraint(is_beq * pc_next);
    yield_constr.constraint(is_beq * pc_branch);
}

#[cfg(test)]
#[allow(clippy::cast_possible_wrap)]
mod test {
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
    }
}

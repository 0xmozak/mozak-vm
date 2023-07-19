use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use starky::constraint_consumer::ConstraintConsumer;

use super::columns::{
    COL_CMP_ABS_DIFF, COL_CMP_DIFF_INV, COL_IMM_VALUE, COL_LESS_THAN_FOR_BRANCH, COL_OP1_VALUE,
    COL_OP2_VALUE, COL_PC, COL_S_BGE, COL_S_BGEU, COL_S_BLT, COL_S_BLTU, NUM_CPU_COLS, OP1_SIGN,
    OP1_VAL_FIXED, OP2_SIGN, OP2_VAL_FIXED,
};

pub(crate) fn constraints<P: PackedField>(
    lv: &[P; NUM_CPU_COLS],
    nv: &[P; NUM_CPU_COLS],
    yield_constr: &mut ConstraintConsumer<P>,
) {
    let p32 = P::Scalar::from_noncanonical_u64(1 << 32);
    let p31 = P::Scalar::from_noncanonical_u64(1 << 31);

    let is_blt = lv[COL_S_BLT];
    let is_bltu = lv[COL_S_BLTU];
    let is_bge = lv[COL_S_BGE];
    let is_bgeu = lv[COL_S_BGEU];
    let is_branch = is_blt + is_bltu + is_bge + is_bgeu;

    let bumped_pc = lv[COL_PC] + P::Scalar::from_noncanonical_u64(4);
    let branched_pc = lv[COL_IMM_VALUE];
    let next_pc = nv[COL_PC];

    let lt = lv[COL_LESS_THAN_FOR_BRANCH];
    yield_constr.constraint(is_branch * (lt * (P::ONES - lt)));

    let sign1 = lv[OP1_SIGN];
    yield_constr.constraint(is_branch * (sign1 * (P::ONES - sign1)));
    let sign2 = lv[OP2_SIGN];
    yield_constr.constraint(is_branch * (sign2 * (P::ONES - sign2)));

    let op1 = lv[COL_OP1_VALUE];
    let op2 = lv[COL_OP2_VALUE];
    // TODO: range check
    let op1_fixed = lv[OP1_VAL_FIXED];
    // TODO: range check
    let op2_fixed = lv[OP2_VAL_FIXED];

    yield_constr.constraint((is_bltu + is_bgeu) * (op1_fixed - op1));
    yield_constr.constraint((is_bltu + is_bgeu) * (op2_fixed - op2));

    yield_constr.constraint((is_blt + is_bge) * (op1_fixed - (op1 + p31 - sign1 * p32)));
    yield_constr.constraint((is_blt + is_bge) * (op2_fixed - (op2 + p31 - sign2 * p32)));

    let diff_fixed = op1_fixed - op2_fixed;
    // TODO: range check
    let abs_diff = lv[COL_CMP_ABS_DIFF];

    // abs_diff calculation
    yield_constr.constraint(is_branch * (P::ONES - lt) * (abs_diff - diff_fixed));
    yield_constr.constraint(is_branch * lt * (abs_diff + diff_fixed));

    let diff = op1 - op2;
    let diff_inv = lv[COL_CMP_DIFF_INV];
    yield_constr.constraint(lt * (P::ONES - diff * diff_inv));

    yield_constr.constraint((is_blt + is_bltu) * lt * (next_pc - branched_pc));
    yield_constr.constraint((is_blt + is_bltu) * (P::ONES - lt) * (next_pc - bumped_pc));

    yield_constr.constraint((is_bge + is_bgeu) * lt * (next_pc - bumped_pc));
    yield_constr.constraint((is_bge + is_bgeu) * (P::ONES - lt) * (next_pc - branched_pc));
}

#[cfg(test)]
#[allow(clippy::cast_possible_wrap)]
mod tests {
    use mozak_vm::instruction::{Args, Instruction, Op};
    use mozak_vm::test_utils::{simple_test_code, u32_extra};
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
        match op {
            Op::BLT =>
                if (a as i32) < (b as i32) {
                    assert_eq!(record.last_state.get_register_value(1), 0);
                } else {
                    assert_eq!(record.last_state.get_register_value(1), 10);
                },
            Op::BLTU =>
                if a < b {
                    assert_eq!(record.last_state.get_register_value(1), 0);
                } else {
                    assert_eq!(record.last_state.get_register_value(1), 10);
                },
            Op::BGE =>
                if (a as i32) >= (b as i32) {
                    assert_eq!(record.last_state.get_register_value(1), 0);
                } else {
                    assert_eq!(record.last_state.get_register_value(1), 10);
                },
            Op::BGEU =>
                if a >= b {
                    assert_eq!(record.last_state.get_register_value(1), 0);
                } else {
                    assert_eq!(record.last_state.get_register_value(1), 10);
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
    }
}

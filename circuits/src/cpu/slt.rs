use plonky2::field::packed::PackedField;
use starky::constraint_consumer::ConstraintConsumer;

use super::columns::{
    COL_CMP_ABS_DIFF, COL_CMP_ABS_DIFF_INV, COL_DST_VALUE, COL_IMM_VALUE, COL_OP1_VALUE,
    COL_OP2_VALUE, COL_S_SLT, COL_S_SLTU, COL_S_SLT_OP1_VAL_FIXED, COL_S_SLT_OP2_VAL_FIXED,
    COL_S_SLT_SIGN1, COL_S_SLT_SIGN2, NUM_CPU_COLS,
};
use super::utils::pc_ticks_up;
use crate::utils::column_of_xs;

pub(crate) fn constraints<P: PackedField>(
    lv: &[P; NUM_CPU_COLS],
    nv: &[P; NUM_CPU_COLS],
    yield_constr: &mut ConstraintConsumer<P>,
) {
    let p32: P = column_of_xs(1 << 32);
    let p31: P = column_of_xs(1 << 31);
    // Watch out: possible values are 0, 1, 2;
    // We only care about zero or non-zero.
    let is_cmp = lv[COL_S_SLT] + lv[COL_S_SLTU];
    let is_signed = lv[COL_S_SLT];

    let lt = lv[COL_DST_VALUE];
    yield_constr.constraint(is_cmp * lt * (P::ONES - lt));

    let sign1 = lv[COL_S_SLT_SIGN1];
    yield_constr.constraint(sign1 * (P::ONES - sign1));
    let sign2 = lv[COL_S_SLT_SIGN2];
    yield_constr.constraint(sign2 * (P::ONES - sign2));

    let op1 = lv[COL_OP1_VALUE];
    let op2 = lv[COL_OP2_VALUE] + lv[COL_IMM_VALUE];
    // TODO: range check
    let op1_fixed = lv[COL_S_SLT_OP1_VAL_FIXED];
    // TODO: range check
    let op2_fixed = lv[COL_S_SLT_OP2_VAL_FIXED];

    yield_constr.constraint(op1_fixed - (op1 - sign1 * p32 + is_signed * p31));
    yield_constr.constraint(op2_fixed - (op2 - sign2 * p32 + is_signed * p31));

    let diff = op1_fixed - op2_fixed;
    // TODO: range check
    let abs_diff = lv[COL_CMP_ABS_DIFF];
    let abs_diff_inv = lv[COL_CMP_ABS_DIFF_INV];

    // abs_diff calculation
    yield_constr.constraint(is_cmp * (P::ONES - lt) * (diff - abs_diff));
    yield_constr.constraint(is_cmp * lt * (-diff - abs_diff));

    // abs_diff * abs_diff_inv = 1 when lt = 1
    yield_constr.constraint(is_cmp * lt * (P::ONES - abs_diff * abs_diff_inv));

    yield_constr.constraint_transition(is_cmp * pc_ticks_up(lv, nv));
}

#[cfg(test)]
#[allow(clippy::cast_possible_wrap)]
mod test {
    use mozak_vm::instruction::{Args, Instruction, Op};
    use mozak_vm::test_utils::{simple_test, simple_test_code};

    use crate::test_utils::simple_proof_test;
    #[test]
    fn prove_add() {
        let record = simple_test(
            4,
            &[(0_u32, 0x0073_02b3 /* add r5, r6, r7 */)],
            &[(6, 100), (7, 100)],
        );
        assert_eq!(record.last_state.get_register_value(5), 100 + 100);
        simple_proof_test(&record.executed).unwrap();
    }
    use proptest::prelude::any;
    use proptest::proptest;
    proptest! {
            #[test]
            fn prove_add_proptest(a in any::<u32>(), b in any::<u32>()) {
                let record = simple_test_code(
                    &[Instruction {
                        op: Op::ADD,
                        args: Args {
                            rd: 5,
                            rs1: 6,
                            rs2: 7,
                            ..Args::default()
                        },
                    }],
                    &[],
                    &[(6, a), (7, b)],
                );
                assert_eq!(record.last_state.get_register_value(5), a.wrapping_add(b));
                simple_proof_test(&record.executed).unwrap();
            }
    }
}

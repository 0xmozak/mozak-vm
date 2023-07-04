use plonky2::field::packed::PackedField;
use starky::constraint_consumer::ConstraintConsumer;

use super::columns::{
    COL_CMP_ABS_DIFF, COL_CMP_DIFF_INV, COL_DST_VALUE, COL_IMM_VALUE, COL_LESS_THAN, COL_OP1_VALUE,
    COL_OP2_VALUE, COL_S_SLT, COL_S_SLTU, COL_S_SLT_OP1_VAL_FIXED, COL_S_SLT_OP2_VAL_FIXED,
    COL_S_SLT_SIGN1, COL_S_SLT_SIGN2, NUM_CPU_COLS,
};
use crate::utils::column_of_xs;

pub(crate) fn constraints<P: PackedField>(
    lv: &[P; NUM_CPU_COLS],
    yield_constr: &mut ConstraintConsumer<P>,
) {
    let p32: P = column_of_xs(1 << 32);
    let p31: P = column_of_xs(1 << 31);

    let is_slt = lv[COL_S_SLT];
    let is_sltu = lv[COL_S_SLTU];
    let is_cmp = is_slt + is_sltu;

    let lt = lv[COL_LESS_THAN];
    yield_constr.constraint(lt * (P::ONES - lt));

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

    yield_constr.constraint(is_sltu * (op1_fixed - op1));
    yield_constr.constraint(is_sltu * (op2_fixed - op2));

    yield_constr.constraint(is_slt * (op1_fixed - (op1 + p31 - sign1 * p32)));
    yield_constr.constraint(is_slt * (op2_fixed - (op2 + p31 - sign2 * p32)));

    let diff_fixed = op1_fixed - op2_fixed;
    // TODO: range check
    let abs_diff = lv[COL_CMP_ABS_DIFF];

    // abs_diff calculation
    yield_constr.constraint(is_cmp * (P::ONES - lt) * (abs_diff - diff_fixed));
    yield_constr.constraint(is_cmp * lt * (abs_diff + diff_fixed));

    let diff = op1 - op2;
    let diff_inv = lv[COL_CMP_DIFF_INV];
    yield_constr.constraint(lt * (P::ONES - diff * diff_inv));
    yield_constr.constraint(is_cmp * (lt - lv[COL_DST_VALUE]));
}

#[cfg(test)]
#[allow(clippy::cast_possible_wrap)]
mod test {
    use mozak_vm::instruction::{Args, Instruction, Op};
    use mozak_vm::test_utils::simple_test_code;
    use proptest::prelude::any;
    use proptest::proptest;

    use crate::test_utils::simple_proof_test;
    proptest! {
            #[test]
            fn prove_slt_proptest(a in any::<u32>(), b in any::<u32>()) {
                let record = simple_test_code(
                    &[Instruction {
                        op: Op::SLTU,
                        args: Args {
                            rd: 5,
                            rs1: 6,
                            rs2: 7,
                            ..Args::default()
                        },
                    },
                    Instruction {
                        op: Op::SLT,
                        args: Args {
                            rd: 4,
                            rs1: 6,
                            rs2: 7,
                            ..Args::default()
                        },
                    }
                    ],
                    &[],
                    &[(6, a), (7, b)],
                );
                assert_eq!(record.last_state.get_register_value(5), (a < b).into());
                assert_eq!(record.last_state.get_register_value(4), ((a as i32) < (b as i32)).into());
                simple_proof_test(&record.executed).unwrap();
            }
    }
}

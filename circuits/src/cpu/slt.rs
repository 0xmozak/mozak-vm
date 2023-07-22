use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use starky::constraint_consumer::ConstraintConsumer;

use super::columns::{
    CMP_ABS_DIFF, CMP_DIFF_INV, DST_VALUE, IMM_VALUE, LESS_THAN, NUM_CPU_COLS, OP1_SIGN, OP1_VALUE,
    OP1_VAL_FIXED, OP2_SIGN, OP2_VALUE, OP2_VAL_FIXED, S_SLT, S_SLTU,
};

pub(crate) fn constraints<P: PackedField>(
    lv: &[P; NUM_CPU_COLS],
    yield_constr: &mut ConstraintConsumer<P>,
) {
    let p32 = P::Scalar::from_noncanonical_u64(1 << 32);
    let p31 = P::Scalar::from_noncanonical_u64(1 << 31);

    let is_slt = lv[S_SLT];
    let is_sltu = lv[S_SLTU];
    let is_cmp = is_slt + is_sltu;

    let lt = lv[LESS_THAN];
    yield_constr.constraint(lt * (P::ONES - lt));

    let sign1 = lv[OP1_SIGN];
    yield_constr.constraint(sign1 * (P::ONES - sign1));
    let sign2 = lv[OP2_SIGN];
    yield_constr.constraint(sign2 * (P::ONES - sign2));

    let op1 = lv.ops.op1_value;
    let op2 = lv.ops.op2_value + lv.imm_value;
    // TODO: range check
    let op1_fixed = lv[OP1_VAL_FIXED];
    // TODO: range check
    let op2_fixed = lv[OP2_VAL_FIXED];

    yield_constr.constraint(is_sltu * (op1_fixed - op1));
    yield_constr.constraint(is_sltu * (op2_fixed - op2));

    yield_constr.constraint(is_slt * (op1_fixed - (op1 + p31 - sign1 * p32)));
    yield_constr.constraint(is_slt * (op2_fixed - (op2 + p31 - sign2 * p32)));

    let diff_fixed = op1_fixed - op2_fixed;
    // TODO: range check
    let abs_diff = lv[CMP_ABS_DIFF];

    // abs_diff calculation
    yield_constr.constraint(is_cmp * (P::ONES - lt) * (abs_diff - diff_fixed));
    yield_constr.constraint(is_cmp * lt * (abs_diff + diff_fixed));

    let diff = op1 - op2;
    let diff_inv = lv[CMP_DIFF_INV];
    yield_constr.constraint(lt * (P::ONES - diff * diff_inv));
    yield_constr.constraint(is_cmp * (lt - lv.dst_value));
}

#[cfg(test)]
#[allow(clippy::cast_possible_wrap)]
mod tests {
    use mozak_vm::instruction::{Args, Instruction, Op};
    use mozak_vm::test_utils::{simple_test_code, u32_extra};
    use proptest::prelude::{any, ProptestConfig};
    use proptest::proptest;

    use crate::test_utils::simple_proof_test;
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(4))]
        #[test]
        fn prove_slt_proptest(a in u32_extra(), op2 in u32_extra(), use_imm in any::<bool>()) {
            let (b, imm) = if use_imm { (0, op2) } else { (op2, 0) };
            let record = simple_test_code(
                &[
                    Instruction {
                        op: Op::SLTU,
                        args: Args {
                            rd: 5,
                            rs1: 6,
                            rs2: 7,
                            imm,
                        },
                    },
                    Instruction {
                        op: Op::SLT,
                        args: Args {
                            rd: 4,
                            rs1: 6,
                            rs2: 7,
                            imm,
                        },
                    },
                ],
                &[],
                &[(6, a), (7, b)],
            );
            assert_eq!(record.last_state.get_register_value(5), (a < op2).into());
            assert_eq!(
                record.last_state.get_register_value(4),
                ((a as i32) < (op2 as i32)).into()
            );
            simple_proof_test(&record.executed).unwrap();
        }
    }
}

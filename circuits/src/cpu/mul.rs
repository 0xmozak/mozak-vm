use plonky2::field::packed::PackedField;
use starky::constraint_consumer::ConstraintConsumer;

use super::columns::{
    COL_DST_VALUE, COL_OP1_VALUE, COL_OP2_VALUE, COL_S_MUL, NUM_CPU_COLS, MUL_HIGH_BITS,
};
use crate::utils::from_;

pub(crate) fn constraints<P: PackedField>(
    lv: &[P; NUM_CPU_COLS],
    yield_constr: &mut ConstraintConsumer<P>,
) {
    let base: P::Scalar = from_(1_u128 << 32);
    let multiplied = lv[COL_OP1_VALUE] * lv[COL_OP2_VALUE];
    let high_part = lv[MUL_HIGH_BITS] * base;

    yield_constr.constraint(lv[COL_S_MUL] * (multiplied - (lv[COL_DST_VALUE] + high_part)));
}

#[cfg(test)]
#[allow(clippy::cast_possible_wrap)]
mod test {
    use mozak_vm::instruction::{Args, Instruction, Op};
    use mozak_vm::test_utils::simple_test_code;
    use proptest::prelude::{any, ProptestConfig};
    use proptest::proptest;

    use crate::test_utils::simple_proof_test;
    proptest! {
            #![proptest_config(ProptestConfig::with_cases(4))]
            #[test]
            fn prove_mul_proptest(a in any::<u32>(), b in any::<u32>(), rd in 0_u8..32) {
                let a = 1;
                let b = 1;
                let record = simple_test_code(
                    &[Instruction {
                        op: Op::MUL,
                        args: Args {
                            rd,
                            rs1: 6,
                            rs2: 7,
                            ..Args::default()
                        },
                    }],
                    &[],
                    &[(6, a), (7, b)],
                );
                // if rd != 0 {
                //     assert_eq!(record.executed[1].state.get_register_value(rd), a.wrapping_mul(b));
                // }
                simple_proof_test(&record.executed).unwrap();
            }
    }
}

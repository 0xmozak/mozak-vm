use plonky2::field::packed::PackedField;
use starky::constraint_consumer::ConstraintConsumer;

use super::columns::{
    COL_DST_VALUE, COL_OP1_VALUE, COL_OP2_VALUE, COL_S_MUL, MUL_HIGH_BITS, MUL_HIGH_DIFF_INV,
    NUM_CPU_COLS,
};
use crate::utils::{column_of_xs, from_};

pub(crate) fn constraints<P: PackedField>(
    lv: &[P; NUM_CPU_COLS],
    yield_constr: &mut ConstraintConsumer<P>,
) {
    // TODO: Need range check for COL_OP1_VALUE, COL_OP2_VALUE and MUL_HIGH_BITS.
    let base: P::Scalar = from_(1_u128 << 32);
    let multiplied = lv[COL_OP1_VALUE] * lv[COL_OP2_VALUE];
    let u32_max = column_of_xs::<P>(u64::from(u32::MAX));
    let diff = u32_max - lv[MUL_HIGH_BITS];
    // MUL_HIGH_BITS should not be equal to u32::MAX
    // as for u32::MAX * u32::MAX MUL_HIGH_BITS will be 0xFFFF_FFFE
    yield_constr.constraint(diff * lv[MUL_HIGH_DIFF_INV] - P::ONES);
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
            #[test]
            fn prove_mul_vivek() {
                let a = 5;
                let b = 4;
                let rd = 5;
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
                    assert_eq!(record.executed[1].state.get_register_value(rd), a.wrapping_mul(b));
                simple_proof_test(&record.executed).unwrap();
            }
    proptest! {
            #![proptest_config(ProptestConfig::with_cases(4))]
            #[test]
            fn prove_mul_proptest(a in any::<u32>(), b in any::<u32>(), rd in 0_u8..32) {
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
                if rd != 0 {
                    assert_eq!(record.executed[1].state.get_register_value(rd), a.wrapping_mul(b));
                }
                simple_proof_test(&record.executed).unwrap();
            }
    }
}

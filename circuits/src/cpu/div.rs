use plonky2::field::packed::PackedField;
use starky::constraint_consumer::ConstraintConsumer;

use super::columns::{
    COL_DST_VALUE, COL_IMM_VALUE, COL_OP1_VALUE, COL_OP2_VALUE, COL_S_DIVU, COL_S_REMU, DIVU_M,
    DIVU_Q_INV, DIVU_R, DIVU_R_TOP, NUM_CPU_COLS,
};
use crate::utils::column_of_xs;

pub(crate) fn constraints<P: PackedField>(
    lv: &[P; NUM_CPU_COLS],
    yield_constr: &mut ConstraintConsumer<P>,
) {
    let p = lv[COL_OP1_VALUE];
    let q = lv[COL_OP2_VALUE] + lv[COL_IMM_VALUE];
    let q_inv = lv[DIVU_Q_INV];
    // TODO: m, r, rt need range-checks.
    let m = lv[DIVU_M];
    let r = lv[DIVU_R];
    // We only need rt column for the range check.
    let rt = lv[DIVU_R_TOP];

    let is_divu = lv[COL_S_DIVU];
    let is_remu = lv[COL_S_REMU];
    let dst = lv[COL_DST_VALUE];

    // Constraints for denominator != 0:
    yield_constr.constraint(q * (m * q + r - p));
    yield_constr.constraint(q * (r + rt - q));

    // Constraints for denominator == 0.  On Risc-V:
    // p / 0 == 0xFFFF_FFFF
    // p % 0 == p
    yield_constr.constraint((P::ONES - q * q_inv) * (m - column_of_xs::<P>(u32::MAX.into())));
    yield_constr.constraint((P::ONES - q * q_inv) * (r - p));

    yield_constr.constraint(is_divu * (dst - m));
    yield_constr.constraint(is_remu * (dst - r));
}

#[cfg(test)]
mod test {
    use mozak_vm::instruction::{Args, Instruction, Op};
    use mozak_vm::test_utils::simple_test_code;
    use proptest::prelude::{any, prop_assert_eq, prop_oneof, Just, ProptestConfig};
    use proptest::{prop_assert, proptest};

    use crate::test_utils::{inv, simple_proof_test};
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]
        #[test]
        fn inv_is_big(x in prop_oneof![Just(0_u32), Just(1_u32), any::<u32>()]) {
            type F = plonky2::field::goldilocks_field::GoldilocksField;
            let y = inv::<F>(u64::from(x));
            if x > 1 {
                prop_assert!(u64::from(u32::MAX) < y);
            }
        }
        #[test]
        fn prove_divu_proptest(p in any::<u32>(), q in prop_oneof![Just(0_u32), Just(1_u32), any::<u32>()], rd in 3_u8..32) {
            let record = simple_test_code(
                &[Instruction {
                    op: Op::DIVU,
                    args: Args {
                        rd,
                        rs1: 1,
                        rs2: 2,
                        ..Args::default()
                    },
                },
                Instruction {
                    op: Op::REMU,
                    args: Args {
                        rd,
                        rs1: 1,
                        rs2: 2,
                        ..Args::default()
                    },
                }
                ],
                &[],
                &[(1, p), (2, q)],
            );
            prop_assert_eq!(record.executed[0].aux.dst_val,
                if let 0 = q {
                    0xffff_ffff
                } else {
                    p / q
                });
            prop_assert_eq!(record.executed[1].aux.dst_val,
                if let 0 = q {
                    p
                } else {
                    p % q
                });
            simple_proof_test(&record.executed).unwrap();
        }
    }
}

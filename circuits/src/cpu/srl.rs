use plonky2::field::packed::PackedField;
use starky::constraint_consumer::ConstraintConsumer;

use super::columns::{
    COL_DST_VALUE, COL_OP1_VALUE, COL_S_SRL, NUM_CPU_COLS, SRL_QUOTIENT, SRL_REMAINDER,
    SRL_REMAINDER_SLACK,
};

pub(crate) fn constraints<P: PackedField>(
    lv: &[P; NUM_CPU_COLS],
    yield_constr: &mut ConstraintConsumer<P>,
) {
    let p = lv[COL_OP1_VALUE];
    // TODO: q needs be checked against lookup table to ensure:
    // q == 1 << shift_amount
    let q = lv[SRL_QUOTIENT];
    // TODO: r needs range-check.
    let r = lv[SRL_REMAINDER];
    // TODO: q_r_1 needs range-check.
    let q_r_1 = lv[SRL_REMAINDER_SLACK];

    let is_srl = lv[COL_S_SRL];
    let dst = lv[COL_DST_VALUE];

    yield_constr.constraint(is_srl * (dst * q + r - p));
    yield_constr.constraint(is_srl * (r + q_r_1 + P::ONES - q));
}

#[cfg(test)]
mod test {
    use mozak_vm::instruction::{Args, Instruction, Op};
    use mozak_vm::test_utils::simple_test_code;
    use proptest::prelude::{any, prop_assert_eq, ProptestConfig};
    use proptest::proptest;

    use crate::test_utils::simple_proof_test;
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(4))]
        #[test]
        fn prove_srl_proptest(p in any::<u32>(), q in 0_u32..32, rd in 3_u8..32) {
            let record = simple_test_code(
                &[Instruction {
                    op: Op::SRL,
                    args: Args {
                        rd,
                        rs1: 1,
                        rs2: 2,
                        ..Args::default()
                    },
                },
                Instruction {
                    op: Op::SRL,
                    args: Args {
                        rd,
                        rs1: 1,
                        rs2: 0,
                        imm: q,
                    },
                }
                ],
                &[],
                &[(1, p), (2, q)],
            );
            prop_assert_eq!(record.executed[0].aux.dst_val, p >> q);
            prop_assert_eq!(record.executed[1].aux.dst_val, p >> q);
            simple_proof_test(&record.executed).unwrap();
        }
    }
}

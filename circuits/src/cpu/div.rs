use plonky2::field::packed::PackedField;
use starky::constraint_consumer::ConstraintConsumer;

use super::columns::{
    COL_DST_VALUE, COL_IMM_VALUE, COL_OP1_VALUE, COL_OP2_VALUE, COL_S_DIVU, COL_S_REMU, COL_S_SRL,
    DIVISOR, DIVISOR_INV, NUM_CPU_COLS, QUOTIENT, REMAINDER, REMAINDER_SLACK,
};
use crate::utils::column_of_xs;

/// Constraints for DIVU / REMU / SRL instructions
///
/// SRL stands for 'shift right logical'.  We can treat it as a variant of
/// unsigned division.
///
/// TODO: m, r, slack need range-checks.
pub(crate) fn constraints<P: PackedField>(
    lv: &[P; NUM_CPU_COLS],
    yield_constr: &mut ConstraintConsumer<P>,
) {
    let is_divu = lv[COL_S_DIVU];
    let is_remu = lv[COL_S_REMU];
    let is_srl = lv[COL_S_SRL];
    let dst = lv[COL_DST_VALUE];

    // https://five-embeddev.com/riscv-isa-manual/latest/m.html says
    // > For both signed and unsigned division, it holds that dividend = divisor ×
    // > quotient + remainder.
    // In the following code, we are looking at p/q.
    let p = lv[COL_OP1_VALUE];
    let op2 = lv[COL_OP2_VALUE] + lv[COL_IMM_VALUE];
    let q = lv[DIVISOR];
    yield_constr.constraint((is_divu + is_remu) * (q - op2));
    // TODO: for SRL `q` needs be checked against lookup table to ensure:
    //     q == 1 << (shift_amount % 0x1F)

    // The equation from the spec becomes:
    //  p = q * m + r
    // (Interestingly, this holds even when q == 0.)
    let m = lv[QUOTIENT];
    let r = lv[REMAINDER];
    yield_constr.constraint(m * q + r - p);

    // However, that constraint is not enough.
    // For example, a malicious prover could trivially fulfill it via
    //  m := 0, r := p

    // The solution is to constrain r further:
    //  0 <= r < q
    // (This only works when q != 0.)

    // Logically, these are two independent constraints:
    //      (A) 0 <= r
    //      (B) r < q
    // Part A is easy: we range-check r.
    // Part B is only slightly harder: borrowing the concept of 'slack variables' from linear programming (https://en.wikipedia.org/wiki/Slack_variable) we get:
    // (B') r + slack + 1 = q
    //      with range_check(slack)

    let slack = lv[REMAINDER_SLACK];
    yield_constr.constraint(q * (r + slack + P::ONES - q));

    // Now we need to deal with division by zero.  The Risc-V spec says:
    //      p / 0 == 0xFFFF_FFFF
    //      p % 0 == p

    let q_inv = lv[DIVISOR_INV];
    yield_constr.constraint((P::ONES - q * q_inv) * (m - column_of_xs::<P>(u32::MAX.into())));
    yield_constr.constraint((P::ONES - q * q_inv) * (r - p));

    // Last, we 'copy' our results:
    yield_constr.constraint((is_divu + is_srl) * (dst - m));
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

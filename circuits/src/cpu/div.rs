use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use starky::constraint_consumer::ConstraintConsumer;

use super::bitwise::and_gadget;
use super::columns::CpuState;

/// Constraints for DIVU / REMU / SRL instructions
///
/// SRL stands for 'shift right logical'.  We can treat it as a variant of
/// unsigned division.
///
/// TODO: m, r, slack need range-checks.
#[allow(clippy::similar_names)]
pub(crate) fn constraints<P: PackedField>(
    lv: &CpuState<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    let dst = lv.dst_value;
    let shifted = CpuState::<P>::shifted;
    let ops = lv.inst.ops;

    // https://five-embeddev.com/riscv-isa-manual/latest/m.html says
    // > For both signed and unsigned division, it holds that dividend = divisor ×
    // > quotient + remainder.
    // In the following code, we are looking at p/q.
    // p,q are between i32::MIN .. u32::MAX
    let p = lv.op1_full_range();
    let q = lv.divisor;
    yield_constr.constraint((ops.div + ops.rem + ops.divu + ops.remu) * (q - lv.op2_full_range()));

    // The following constraints are for SRL.
    {
        let and_gadget = and_gadget(&lv.xor);
        yield_constr
            .constraint(ops.srl * (and_gadget.input_a - P::Scalar::from_noncanonical_u64(0b1_1111)));
        let op2 = lv.op2_value;
        yield_constr.constraint(ops.srl * (and_gadget.input_b - op2));

        yield_constr.constraint(ops.srl * (and_gadget.output - lv.bitshift.amount));
        yield_constr.constraint(ops.srl * (q - lv.bitshift.multiplier));
    }

    // TODO(Matthias): this looks suspicious in the face of signed bit shifting
    // (SRA)
    let q_sign = P::ONES - lv.op2_sign_bit.doubles();

    // Watch out for sign!
    let q_inv = lv.divisor_inv;
    // TODO: m_abs, r_abs, rt need range-checks.
    let m = lv.quotient;
    let m_abs = lv.quotient_abs;
    yield_constr.constraint((m - m_abs) * (m + m_abs));
    let r = lv.remainder;
    let r_abs = lv.remainder_abs;
    yield_constr.constraint((r - r_abs) * (r + r_abs));
    // We only need rt column to range-check rt := q - r
    let rt = lv.remainder_abs_slack;

    // The equation from the spec becomes:
    //  p = q * m + r
    // (Interestingly, this holds even when q == 0.)
    // TODO(Matthias): the above observation is from the spec, but why do we need to
    // treat 0 special in the line below?

    // Constraints for denominator != 0:
    yield_constr.constraint(q * (m * q + r - p));
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
    yield_constr.constraint(q * (r_abs + rt + P::ONES - q * q_sign));

    // Now we need to deal with division by zero.  The Risc-V spec says:
    //      p / 0 == 0xFFFF_FFFF
    //      p % 0 == p
    yield_constr.constraint((P::ONES - q * q_inv) * (m - P::Scalar::from_canonical_u32(u32::MAX)));
    yield_constr.constraint((P::ONES - q * q_inv) * (r - lv.op1_value));

    // Last, we 'copy' our results:
    yield_constr.constraint((ops.divu + ops.srl) * (dst - m));
    yield_constr.constraint(ops.div * (dst - m) * (dst - m - shifted(32)));

    yield_constr.constraint(ops.remu * (dst - r));
    yield_constr.constraint(ops.rem * (dst - r) * (dst - r - shifted(32)));
}

#[cfg(test)]
mod tests {
    use mozak_vm::instruction::{Args, Instruction, Op};
    use mozak_vm::test_utils::{simple_test_code, u32_extra};
    use proptest::prelude::{prop_assert_eq, ProptestConfig};
    use proptest::{prop_assert, proptest};

    use crate::cpu::stark::CpuStark;
    use crate::test_utils::{inv, ProveAndVerify};
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(4))]
        #[test]
        fn inv_is_big(x in u32_extra()) {
            type F = plonky2::field::goldilocks_field::GoldilocksField;
            let y = inv::<F>(u64::from(x));
            if x > 1 {
                prop_assert!(u64::from(u32::MAX) < y);
            }
        }
        #[test]
        fn prove_divu_proptest(p in u32_extra(), q in u32_extra(), rd in 3_u8..32) {
            let (program, record) = simple_test_code(
                &[Instruction {
                    op: Op::DIVU,
                    args: Args {
                        rd,
                        rs1: 1,
                        rs2: 2,
                        ..Args::default()
                    },
                },
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
            CpuStark::prove_and_verify(&program, &record.executed).unwrap();
        }
        #[test]
        #[allow(clippy::cast_sign_loss)]
        #[allow(clippy::cast_possible_wrap)]
        fn prove_div_proptest(p in u32_extra(), q in u32_extra(), rd in 3_u8..32) {
            let (program, record) = simple_test_code(
                &[Instruction {
                    op: Op::DIV,
                    args: Args {
                        rd,
                        rs1: 1,
                        rs2: 2,
                        ..Args::default()
                    },
                },
                ],
                &[],
                &[(1, p), (2, q)],
            );
            prop_assert_eq!(record.executed[0].aux.dst_val,
                if let 0 = q {
                    0xffff_ffff
                } else {
                    (p as i32).wrapping_div(q as i32) as u32
                });
            CpuStark::prove_and_verify(&program, &record.executed).unwrap();
        }
        #[test]
        fn prove_remu_proptest(p in u32_extra(), q in u32_extra(), rd in 3_u8..32) {
            let (program, record) = simple_test_code(
                &[
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
                    p
                } else {
                    p % q
                });
            CpuStark::prove_and_verify(&program, &record.executed).unwrap();
        }
        #[allow(clippy::cast_sign_loss)]
        #[allow(clippy::cast_possible_wrap)]
        #[test]
        fn prove_rem_proptest(p in u32_extra(), q in u32_extra(), rd in 3_u8..32) {
            let (program, record) = simple_test_code(
                &[
                Instruction {
                    op: Op::REM,
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
                    p
                } else {
                    (p as i32).wrapping_rem(q as i32) as u32
                });
            CpuStark::prove_and_verify(&program, &record.executed).unwrap();
        }
        #[test]
        fn prove_srl_proptest(p in u32_extra(), q in 0_u32..32, rd in 3_u8..32) {
            let (program, record) = simple_test_code(
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
                        imm: q,
                        ..Args::default()
                    },
                }
                ],
                &[],
                &[(1, p), (2, q)],
            );
            prop_assert_eq!(record.executed[0].aux.dst_val, p >> q);
            prop_assert_eq!(record.executed[1].aux.dst_val, p >> q);
            CpuStark::prove_and_verify(&program, &record.executed).unwrap();
        }
    }
}

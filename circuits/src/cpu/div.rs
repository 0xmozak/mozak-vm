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
    let shifted = CpuState::<P>::shifted;
    let is_divu = lv.inst.ops.divu;
    let is_remu = lv.inst.ops.remu;
    let is_div = lv.inst.ops.div;
    let is_rem = lv.inst.ops.rem;
    let is_srl = lv.inst.ops.srl;
    let is_sra = lv.inst.ops.sra;

    // p,q are between i32::MIN .. u32::MAX
    let p = lv.op1_full_range();
    let q = lv.divisor;

    let p_raw = lv.op1_value;
    let q_raw = lv.op2_value;

    let q_sign = P::Scalar::from_noncanonical_i64(-2) * lv.op2_sign_bit + P::ONES;

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

    yield_constr.constraint((is_divu + is_remu) * (lv.divisor - q_raw));
    yield_constr.constraint((is_div + is_rem) * (lv.divisor - lv.op2_full_range()));

    let dst = lv.dst_value;
    // The following constraints are for SRL/SRA.
    {
        let and_gadget = and_gadget(&lv.xor);
        yield_constr.constraint(
            (is_srl + is_sra) * (and_gadget.input_a - P::Scalar::from_noncanonical_u64(0x1F)),
        );
        let op2 = lv.op2_value;
        yield_constr.constraint((is_srl + is_sra) * (and_gadget.input_b - op2));

        yield_constr.constraint((is_srl + is_sra) * (and_gadget.output - lv.bitshift.amount));
        yield_constr.constraint((is_srl + is_sra) * (lv.divisor - lv.bitshift.multiplier));
    }

    // https://five-embeddev.com/riscv-isa-manual/latest/m.html says
    // > For both signed and unsigned division, it holds that dividend = divisor ×
    // > quotient + remainder.
    // The equation from the spec becomes:
    //  p = q * m + r
    // (Interestingly, this holds even when q == 0.)
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

    // Constraints for denominator == 0.  On Risc-V:
    // p / 0 == 0xFFFF_FFFF
    // p % 0 == p
    yield_constr.constraint((P::ONES - q * q_inv) * (m - P::Scalar::from_canonical_u32(u32::MAX)));
    yield_constr.constraint((P::ONES - q * q_inv) * (r - p_raw));

    // Last, we 'copy' our results:
    yield_constr.constraint((is_divu + is_srl) * (dst - m));
    yield_constr.constraint(is_div * (dst - m) * (dst - m - shifted(32)));
    // TODO (Vivek): Following constraint is degree 4, why it is not getting error
    // as CPU constraint degreee is 3?
    yield_constr.constraint(
        is_sra
            * (dst - m - (shifted(32) * lv.op1_sign_bit))
            * (dst - P::Scalar::from_canonical_u32(u32::MAX)),
    );

    yield_constr.constraint(is_remu * (dst - r));
    yield_constr.constraint(is_rem * (dst - r) * (dst - r - shifted(32)));
}

#[cfg(test)]
mod tests {
    use mozak_vm::instruction::{Args, Instruction, Op};
    use mozak_vm::test_utils::{simple_test_code, u32_extra};
    use proptest::prelude::{prop_assert_eq, ProptestConfig};
    use proptest::{prop_assert, proptest};

    use crate::cpu::stark::CpuStark;
    use crate::test_utils::{inv, ProveAndVerify};
    #[test]
    #[allow(clippy::cast_sign_loss)]
    #[allow(clippy::cast_possible_wrap)]
    fn prove_div_example() {
        // let p = u32::MAX;
        // let q = 2;
        let p = 0x8000_0000;
        let q = 1;
        let (program, record) = simple_test_code(
            &[Instruction {
                op: Op::DIV,
                args: Args {
                    rd: 3,
                    rs1: 1,
                    rs2: 2,
                    ..Args::default()
                },
            }],
            &[],
            &[(1, p), (2, q)],
        );
        assert_eq!(
            record.executed[0].aux.dst_val,
            ((p as i32).wrapping_div(q as i32)) as u32
        );
        CpuStark::prove_and_verify(&program, &record.executed).unwrap();
    }
    #[test]
    #[allow(clippy::cast_sign_loss)]
    #[allow(clippy::cast_possible_wrap)]
    fn prove_sra_example() {
        // let p =  u32::MAX;
        // let q = 1;
        // let p =  0xFFFF_FFF4;
        // let q = 4;
        let p = 0x8000_0000;
        let q = 0;
        let (program, record) = simple_test_code(
            &[
                Instruction {
                    op: Op::SRA,
                    args: Args {
                        rd: 3,
                        rs1: 1,
                        rs2: 2,
                        ..Args::default()
                    },
                },
                Instruction {
                    op: Op::SRA,
                    args: Args {
                        rd: 3,
                        rs1: 1,
                        imm: q,
                        ..Args::default()
                    },
                },
            ],
            &[],
            &[(1, p), (2, q)],
        );
        assert_eq!(
            record.executed[0].aux.dst_val,
            ((p as i32) >> (q as i32)) as u32
        );
        assert_eq!(
            record.executed[1].aux.dst_val,
            ((p as i32) >> (q as i32)) as u32
        );
        CpuStark::prove_and_verify(&program, &record.executed).unwrap();
    }
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
        #[test]
    #[allow(clippy::cast_sign_loss)]
    #[allow(clippy::cast_possible_wrap)]
        fn prove_sra_proptest(p in u32_extra(), q in 0_u32..32, rd in 3_u8..32) {
            let (program, record) = simple_test_code(
                &[Instruction {
                    op: Op::SRA,
                    args: Args {
                        rd,
                        rs1: 1,
                        rs2: 2,
                        ..Args::default()
                    },
                },
                Instruction {
                    op: Op::SRA,
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
            prop_assert_eq!(record.executed[0].aux.dst_val, ((p as i32) >> (q as i32)) as u32);
            prop_assert_eq!(record.executed[1].aux.dst_val, ((p as i32) >> (q as i32)) as u32);
            CpuStark::prove_and_verify(&program, &record.executed).unwrap();
        }
    }
}

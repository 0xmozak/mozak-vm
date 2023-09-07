//! This module implements constraints for division operations, including
//! DIVU, REMU, DIV, REM and SRL instructions.
//!
//! Here, SRL stands for 'shift right logical'.  We can treat it as a variant of
//! unsigned multiplication.

use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use starky::constraint_consumer::ConstraintConsumer;

use super::bitwise::and_gadget;
use super::columns::CpuState;

/// Constraints for DIV / REM / DIVU / REMU / SRL instructions
///
/// SRL stands for 'shift right logical'.  We can treat it as a variant of
/// unsigned division.
#[allow(clippy::similar_names)]
pub(crate) fn constraints<P: PackedField>(
    lv: &CpuState<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    let two_to_32 = CpuState::<P>::shifted(32);
    let is_divu = lv.inst.ops.divu;
    let is_remu = lv.inst.ops.remu;
    let is_div = lv.inst.ops.div;
    let is_rem = lv.inst.ops.rem;
    let is_srl = lv.inst.ops.srl;

    // Note that we are using the following columns that also used in MUL
    // constraints. Also note that the overflow case (-2^31 / -1) is also
    // handled in MUL constraints.
    let quotient = lv.op1_value;
    // let quotient_sign = lv.op1_sign_bit;
    let quotient_abs = lv.op1_abs;
    let divisor = lv.op2_value;
    // let divisor_sign = lv.op2_sign_bit;
    let divisor_inv = lv.op2_inv;
    let divisor_abs = lv.op2_abs;
    let dividend = lv.product_low_limb;
    let dividend_remainder_sign = lv.product_sign;
    // The following columns are used only in this function:
    let dividend_abs = lv.dividend_abs;
    let remainder_abs = lv.remainder_abs;

    // For DIV operations rs1 value is loaded into dividend column.
    // Checks dividend (product_low_limb) is loaded correctly.
    let rs1_value = (0..32)
        .map(|reg| lv.inst.rs1_select[reg] * lv.regs[reg])
        .sum::<P>();
    yield_constr.constraint(dividend - rs1_value);

    // Checks dividend_abs is set correctly.
    yield_constr.constraint(dividend_remainder_sign * (dividend_abs - dividend));
    yield_constr
        .constraint((P::ONES - dividend_remainder_sign) * (two_to_32 - dividend_abs - dividend));

    // https://five-embeddev.com/riscv-isa-manual/latest/m.html says
    // > For both signed and unsigned division, it holds that
    // > |dividend| = |divisor| × |quotient| + |remainder|.
    yield_constr.constraint(divisor_abs * quotient_abs + remainder_abs - dividend_abs);

    // However, that constraint is not enough.
    // For example, a malicious prover could trivially fulfill it via
    //  quotient := 0, r (remainder) := p (dividend)
    // The solution is to constrain r further:
    //  0 <= r < q (divisor)
    // (This only works when q != 0.)
    // Logically, these are two independent constraints:
    //      (A) 0 <= r
    //      (B) r < q
    // Part A is easy: we range-check r.
    // Part B is only slightly harder: borrowing the concept of 'slack variables' from linear programming (https://en.wikipedia.org/wiki/Slack_variable) we get:
    // (B') r + slack + 1 = q
    //      with range_check(slack)
    yield_constr.constraint(divisor_abs * (P::ONES + lv.remainder_slack - divisor_abs));

    // Constraints for divisor == 0.  On Risc-V:
    // p / 0 == 0xFFFF_FFFF
    // p % 0 == p
    yield_constr.constraint(
        (P::ONES - divisor_abs * divisor_inv)
            * (quotient - P::Scalar::from_canonical_u32(u32::MAX)),
    );
    yield_constr.constraint((P::ONES - divisor_abs * divisor_inv) * (remainder_abs - divisor_abs));

    // Check: for SRL, 'divisor' is assigned as `2^(op2 & 0b1_111)`.
    // We only take lowest 5 bits of the op2 for the shift amount.
    // This is following the RISC-V specification.
    // Bellow we use the And gadget to calculate the shift amount, and then use
    // Bitshift table to retrieve the corresponding power of 2, that we will assign
    // to the multiplier.
    {
        let and_gadget = and_gadget(&lv.xor);
        yield_constr
            .constraint(is_srl * (and_gadget.input_a - P::Scalar::from_noncanonical_u64(0b1_1111)));
        let op2 = lv.op2_value;
        yield_constr.constraint(is_srl * (and_gadget.input_b - op2));

        yield_constr.constraint(is_srl * (and_gadget.output - lv.bitshift.amount));
        yield_constr.constraint(is_srl * (divisor - lv.bitshift.multiplier));
    }

    // Last, we 'copy' our results:
    let dst = lv.dst_value;
    yield_constr.constraint((is_div + is_divu + is_srl) * (dst - quotient));
    yield_constr.constraint(is_remu * (dst - remainder_abs));
    yield_constr.constraint(is_rem * dividend_remainder_sign * (two_to_32 - remainder_abs - dst));
    yield_constr.constraint(is_rem * (P::ONES - dividend_remainder_sign) * (remainder_abs - dst));
}

#[cfg(test)]
mod tests {
    use mozak_vm::instruction::{Args, Instruction, Op};
    use mozak_vm::test_utils::{simple_test_code, u32_extra};
    use proptest::prelude::{prop_assert_eq, ProptestConfig};
    use proptest::{prop_assert, proptest};

    use crate::cpu::stark::CpuStark;
    use crate::stark::mozak_stark::MozakStark;
    use crate::test_utils::{inv, ProveAndVerify};

    fn divu_remu_instructions(rd: u8) -> [Instruction; 2] {
        [
            Instruction {
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
            },
        ]
    }

    fn srl_instructions(rd: u8, q: u32) -> [Instruction; 2] {
        [
            Instruction {
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
            },
        ]
    }

    fn div_rem_instructions(rd: u8) -> [Instruction; 2] {
        [
            Instruction {
                op: Op::DIV,
                args: Args {
                    rd,
                    rs1: 1,
                    rs2: 2,
                    ..Args::default()
                },
            },
            Instruction {
                op: Op::REM,
                args: Args {
                    rd,
                    rs1: 1,
                    rs2: 2,
                    ..Args::default()
                },
            },
        ]
    }

    #[test]
    fn prove_divu_remu() {
        let (program, record) =
            simple_test_code(&divu_remu_instructions(3), &[], &[(1, 200), (2, 100)]);
        MozakStark::prove_and_verify(&program, &record).unwrap();
    }

    #[test]
    fn prove_div_rem() {
        let (program, record) =
            simple_test_code(&div_rem_instructions(3), &[], &[(1, 200), (2, 100)]);
        MozakStark::prove_and_verify(&program, &record).unwrap();
    }

    #[test]
    fn prove_srl() {
        let (program, record) =
            simple_test_code(&srl_instructions(3, 200), &[], &[(1, 200), (2, 100)]);
        MozakStark::prove_and_verify(&program, &record).unwrap();
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
                &divu_remu_instructions(rd),
                &[],
                &[(1, p), (2, q)],
            );
            prop_assert_eq!(record.executed[0].aux.dst_val,
                if let 0 = q {
                    p
                } else {
                    p % q
                });
            CpuStark::prove_and_verify(&program, &record).unwrap();
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
            CpuStark::prove_and_verify(&program, &record).unwrap();
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
            CpuStark::prove_and_verify(&program, &record).unwrap();
        }

        #[test]
        fn prove_srl_proptest(p in u32_extra(), q in 0_u32..32, rd in 3_u8..32) {
            let (program, record) = simple_test_code(
                &srl_instructions(rd, q),
                &[],
                &[(1, p), (2, q)],
            );
            prop_assert_eq!(record.executed[0].aux.dst_val, p >> q);
            prop_assert_eq!(record.executed[1].aux.dst_val, p >> q);
            CpuStark::prove_and_verify(&program, &record).unwrap();
        }
    }
}

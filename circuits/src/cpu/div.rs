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
use crate::cpu::stark::is_binary;

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
    let any_div = is_divu + is_remu + is_div + is_rem;
    let all = is_divu + is_remu + is_div + is_rem + is_srl;

    // Note that we are reusing the following columns that also used in MUL
    // constraints. The overflow case (-2^31 / -1) is taken care by MUL constraints.
    let quotient = lv.op1_value;
    let quotient_abs = lv.op1_abs;
    let divisor = lv.op2_value;
    let divisor_abs = lv.op2_abs;
    // The following columns are used only in this function:
    let dividend_sign = lv.dividend_sign;
    let divisor_zero = lv.op2_zero;
    let divisor_inv = lv.op2_inv;
    let dividend_abs = lv.dividend_abs;
    let remainder_abs = lv.remainder_abs;
    let remainder_sign = lv.remainder_sign;

    // Check dividend_sign and reminder_sign are set correctly.
    is_binary(yield_constr, remainder_sign);
    is_binary(yield_constr, dividend_sign);
    yield_constr
        .constraint(lv.product_low_limb * remainder_abs * (lv.product_sign - dividend_sign));
    yield_constr.constraint(dividend_abs * remainder_abs * (dividend_sign - remainder_sign));

    // For DIV operations rs1 value is loaded into dividend column.
    // Checks dividend_abs is set correctly.
    let dividend = (0..32)
        .map(|reg| lv.inst.rs1_select[reg] * lv.regs[reg])
        .sum::<P>();
    yield_constr.constraint((P::ONES - dividend_sign) * (dividend_abs - dividend));
    yield_constr.constraint(dividend_sign * (two_to_32 - dividend_abs - dividend));

    // https://five-embeddev.com/riscv-isa-manual/latest/m.html says
    // > For both signed and unsigned division, it holds that
    // > |dividend| = |divisor| × |quotient| + |remainder|.
    yield_constr.constraint(all * (divisor_abs * quotient_abs + remainder_abs - dividend_abs));

    // However, that constraint is not enough.
    // For example, a malicious prover could trivially fulfill it via
    //  quotient := 0, remainder := dividend
    // The solution is to constrain remainder further:
    //  0 <= remainder < divisor
    // (This only works when divisor != 0.)
    // Logically, these are two independent constraints:
    //      (A) 0 <= remainder
    //      (B) remainder < divisor
    // Part A is easy: we range-check remainder.
    // Part B is only slightly harder: borrowing the concept of 'slack variables' from linear programming (https://en.wikipedia.org/wiki/Slack_variable) we get:
    // (B') remainder + slack + 1 = divisor
    //      with range_check(slack)
    yield_constr.constraint(
        all * divisor_abs * (remainder_abs + P::ONES + lv.remainder_slack - divisor_abs),
    );

    // Constraints for divisor == 0.  On Risc-V:
    // p / 0 == 0xFFFF_FFFF
    // p % 0 == p
    is_binary(yield_constr, divisor_zero);
    yield_constr.constraint(P::ONES - divisor * divisor_inv - divisor_zero);
    yield_constr
        .constraint(any_div * divisor_zero * (quotient - P::Scalar::from_canonical_u32(u32::MAX)));
    yield_constr.constraint(any_div * divisor_zero * (remainder_abs - dividend_abs));
    yield_constr.constraint(any_div * divisor_zero * (remainder_sign - dividend_sign));

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
        yield_constr.constraint(is_srl * (lv.op2_abs - lv.bitshift.multiplier));
    }

    // Last, we 'copy' our results:
    let dst = lv.dst_value;
    yield_constr.constraint((is_div + is_divu + is_srl) * (dst - quotient));
    yield_constr.constraint(is_rem * remainder_sign * (two_to_32 - remainder_abs - dst));
    yield_constr.constraint(is_rem * (P::ONES - remainder_sign) * (remainder_abs - dst));
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use mozak_vm::instruction::{Args, Instruction, Op};
    use mozak_vm::test_utils::{simple_test_code, u32_extra};
    use proptest::prelude::{prop_assert_eq, ProptestConfig};
    use proptest::test_runner::TestCaseError;
    use proptest::{prop_assert, proptest};

    use crate::cpu::stark::CpuStark;
    use crate::stark::mozak_stark::MozakStark;
    use crate::test_utils::{inv, ProveAndVerify, D, F};

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

    fn prove_divu<Stark: ProveAndVerify>(p: u32, q: u32, rd: u8) -> Result<(), TestCaseError> {
        let (program, record) =
            simple_test_code(&divu_remu_instructions(rd), &[], &[(1, p), (2, q)]);
        prop_assert_eq!(
            record.executed[0].aux.dst_val,
            if let 0 = q { 0xffff_ffff } else { p / q }
        );
        prop_assert_eq!(
            record.executed[1].aux.dst_val,
            if let 0 = q { p } else { p % q }
        );
        Stark::prove_and_verify(&program, &record).unwrap();
        Ok(())
    }

    fn prove_srl<Stark: ProveAndVerify>(p: u32, q: u32, rd: u8) -> Result<(), TestCaseError> {
        let (program, record) = simple_test_code(&srl_instructions(rd, q), &[], &[(1, p), (2, q)]);
        prop_assert_eq!(record.executed[0].aux.dst_val, p >> q);
        prop_assert_eq!(record.executed[1].aux.dst_val, p >> q);
        Stark::prove_and_verify(&program, &record).unwrap();
        Ok(())
    }

    fn prove_div<Stark: ProveAndVerify>(p: u32, q: u32, rd: u8) -> Result<(), TestCaseError> {
        let (program, record) = simple_test_code(&div_rem_instructions(rd), &[], &[(1, p), (2, q)]);
        Stark::prove_and_verify(&program, &record).unwrap();
        Ok(())
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
        fn prove_div_cpu(p in u32_extra(), q in u32_extra(), rd in 3_u8..32) {
            prove_div::<CpuStark<F, D>>(p, q, rd)?;
        }

        #[test]
        fn prove_divu_cpu(p in u32_extra(), q in u32_extra(), rd in 3_u8..32) {
            prove_divu::<CpuStark<F, D>>(p, q, rd)?;
        }

        #[test]
        fn prove_srl_cpu(p in u32_extra(), q in 0_u32..32, rd in 3_u8..32) {
            prove_srl::<CpuStark<F, D>>(p, q, rd)?;
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(1))]

        #[test]
        fn prove_div_mozak(p in u32_extra(), q in u32_extra(), rd in 3_u8..32) {
            prove_div::<MozakStark<F, D>>(p, q, rd)?;
        }

        #[test]
        fn prove_divu_mozak(p in u32_extra(), q in u32_extra(), rd in 3_u8..32) {
            prove_divu::<MozakStark<F, D>>(p, q, rd)?;
        }

        #[test]
        fn prove_srl_mozak(p in u32_extra(), q in 0_u32..32, rd in 3_u8..32) {
            prove_srl::<MozakStark<F, D>>(p, q, rd)?;
        }
    }
}

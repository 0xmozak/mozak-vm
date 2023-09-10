//! This module implements constraints for division operations, including
//! DIVU, REMU, DIV, REM and SRL instructions.
//!
//! Here, SRL stands for 'shift right logical'.  We can treat it as a variant of
//! unsigned multiplication.

use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use starky::constraint_consumer::ConstraintConsumer;

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
    // let all = is_divu + is_remu + is_div + is_rem + is_srl;

    let dividend_value = lv.op1_value;
    let dividend_sign = lv.op1_sign_bit;
    let dividend_full_range = CpuState::<P>::op1_full_range(lv);
    let divisor_value = lv.op2_value;
    let divisor_abs = lv.op2_abs;
    let divisor_full_range = CpuState::<P>::op2_full_range(lv);

    // The following columns are used only in this function:
    let divisor_value_inv = lv.op2_value_inv;
    let quotient_value = lv.quotient_value;
    let quotient_sign = lv.quotient_sign;
    let remainder_value = lv.remainder_value;
    let remainder_sign = lv.remainder_sign;
    let remainder_slack = lv.remainder_slack;

    // The range checks that constrain quotient_sign and remainder_sign can be found
    // in cpu.rs.
    is_binary(yield_constr, remainder_sign);
    is_binary(yield_constr, dividend_sign);

    // https://five-embeddev.com/riscv-isa-manual/latest/m.html says
    // > For both signed and unsigned division, it holds that
    // > dividend = divisor × quotient + remainder.
    let quotient_full_range = quotient_value - quotient_sign * two_to_32;
    let remainder_full_range = remainder_value - remainder_sign * two_to_32;
    yield_constr.constraint(
        divisor_full_range
            * (divisor_full_range * quotient_full_range + remainder_full_range
                - dividend_full_range),
    );

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
    let remainder_abs =
        (P::ONES - remainder_sign * P::Scalar::from_canonical_u32(2)) * remainder_full_range;
    yield_constr
        .constraint(divisor_abs * (remainder_abs + P::ONES + remainder_slack - divisor_abs));

    // Constraints for divisor == 0.  On Risc-V:
    // p / 0 == 0xFFFF_FFFF
    // p % 0 == p
    yield_constr.constraint(
        (P::ONES - divisor_value * divisor_value_inv)
            * (quotient_value - P::Scalar::from_canonical_u32(u32::MAX)),
    );
    yield_constr.constraint(
        (P::ONES - divisor_value * divisor_value_inv) * (remainder_value - dividend_value),
    );

    // Last, we 'copy' our results:
    let dst = lv.dst_value;
    yield_constr.constraint((is_div + is_divu + is_srl) * (dst - quotient_value));
    yield_constr.constraint((is_rem + is_remu) * (dst - remainder_value));
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

    fn prove_div<Stark: ProveAndVerify>(p: u32, q: u32, rd: u8) -> Result<(), TestCaseError> {
        let (program, record) = simple_test_code(&div_rem_instructions(rd), &[], &[(1, p), (2, q)]);
        Stark::prove_and_verify(&program, &record).unwrap();
        Ok(())
    }

    #[test]
    fn prove_div_rem_example() {
        let (program, record) = simple_test_code(&div_rem_instructions(3), &[], &[(1, 1), (2, 0)]);
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
        fn prove_div_cpu(p in u32_extra(), q in u32_extra(), rd in 3_u8..32) {
            prove_div::<CpuStark<F, D>>(p, q, rd)?;
        }

        #[test]
        fn prove_divu_cpu(p in u32_extra(), q in u32_extra(), rd in 3_u8..32) {
            prove_divu::<CpuStark<F, D>>(p, q, rd)?;
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

    }
}

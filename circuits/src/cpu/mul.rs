//! This module implements constraints for multiplication operations, including
//! MUL, MULH, MULHU, MULHSU and SLL instructions.
//!
//! Here, SLL stands for 'shift left logical'.  We can treat it as a variant of
//! unsigned multiplication.

use expr::Expr;

use super::columns::CpuState;
use crate::expr::ConstraintBuilder;

/// Converts from a sign-bit to a multiplicative sign.
///
/// Specifically, if `sign_bit` is 0, returns 1.
/// And if `sign_bit` is 1, returns -1.
/// Undefined for any other input.
#[must_use]
pub fn bit_to_sign<P: Copy>(sign_bit: Expr<'_, P>) -> Expr<'_, P> { 1 - 2 * sign_bit }

pub(crate) fn constraints<'a, P: Copy>(
    lv: &CpuState<Expr<'a, P>>,
    cb: &mut ConstraintBuilder<Expr<'a, P>>,
) {
    let op1_abs = lv.op1_abs;
    let op2_abs = lv.op2_abs;
    let low_limb = lv.product_low_limb;
    let high_limb = lv.product_high_limb;
    let product_sign = lv.product_sign;

    // Make sure product_sign is either 0 or 1.
    cb.always(product_sign.is_binary());

    // Ensure correct computation of op1_abs * op2_abs using low_limb and high_limb.
    // If product_sign is 1, verify using the 2's complement: 2^64 - (high_limb *
    // 2^32 + low_limb).
    cb.always((1 - product_sign) * (high_limb * (1 << 32) + low_limb - op1_abs * op2_abs));
    cb.always(product_sign * ((1 << 32) * ((1 << 32) - high_limb) - low_limb - op1_abs * op2_abs));

    // The constraints above would be enough, if our field was large enough.
    // However Goldilocks field is just a bit too small at order 2^64 - 2^32 + 1,
    // which allows for the following exploit to happen e.g. when high_limb ==
    // u32::MAX
    //
    // Specifically, (1<<32) * (u32::MAX) === -1 (mod 2^64 - 2^32 + 1).
    // Thus, when product high_limb == u32::MAX:
    //       product = low_limb + (1 << 32) * high_limb =
    //       = low_limb + (1<<32) * (u32::MAX) = low_limb - 1
    //
    // Which means a malicious prover could evaluate some product in two different
    // ways, which is unacceptable.
    //
    // However, the largest combined result that an honest prover can produce is
    // u32::MAX * u32::MAX = 0xFFFF_FFFE_0000_0001.  So we can add a always
    // that high_limb is != 0xFFFF_FFFF == u32::MAX range to prevent such exploit.

    // Make sure high_limb is not 0xFFFF_FFFF when product_sign is 0 to avoid
    // overflow.
    cb.always(
        (1 - product_sign) * (1 - (0xffff_ffff - high_limb) * lv.product_high_limb_inv_helper),
    );
    // Make sure ((1 << 32) - high_limb) is not 0xFFFF_FFFF when product_sign is 1
    // to avoid overflow of (1 << 32) * ((1 << 32) - high_limb) in the above
    // constraints.
    cb.always(product_sign * (1 - high_limb * lv.product_high_limb_inv_helper));

    // Make sure op1_abs is computed correctly from op1_value.
    cb.always(op1_abs - lv.op1_full_range() * bit_to_sign(lv.op1_sign_bit));

    // Make sure op2_abs is computed correctly from op2_value for MUL operations.
    cb.always(op2_abs - lv.op2_full_range() * bit_to_sign(lv.op2_sign_bit));

    // If both factors are unsigned, the output will always be
    // non-negative/unsigned. As an optimization, we take advantage of the fact
    // that is_op1_signed == 0 implies is_op2_signed == 0 for all our operations.
    // (In fact, the two values only differ for MULHSU.)
    cb.always((1 - lv.inst.is_op1_signed) * product_sign);

    // Ensure skip_check_product_sign can be set to 1 only when either ob1_abs or
    // op2_abs is 0. This check is essential for the subsequent constraints.
    // We are not concerned with other values of skip_check_product_sign.
    cb.always(lv.skip_check_product_sign * lv.op1_abs * lv.op2_abs);

    // Make sure product_sign is computed correctly.
    cb.always(
        (1 - lv.skip_check_product_sign)
            * (bit_to_sign(product_sign)
                - bit_to_sign(lv.op1_sign_bit) * bit_to_sign(lv.op2_sign_bit)),
    );

    // Now, check, that we select the correct output based on the opcode.
    let destination = lv.dst_value;
    cb.always((lv.inst.ops.mul + lv.inst.ops.sll) * (destination - low_limb));
    cb.always((lv.inst.ops.mulh) * (destination - high_limb));
}

#[cfg(test)]
mod tests {

    use std::borrow::Borrow;

    use anyhow::Result;
    use mozak_runner::code;
    use mozak_runner::instruction::{Args, Instruction, Op};
    use mozak_runner::test_utils::{i32_extra, u32_extra};
    use plonky2::timed;
    use plonky2::util::timing::TimingTree;
    use proptest::prelude::ProptestConfig;
    use proptest::test_runner::TestCaseError;
    use proptest::{prop_assert_eq, proptest};
    use starky::prover::prove as prove_table;
    use starky::verifier::verify_stark_proof;

    use crate::cpu::generation::generate_cpu_trace;
    use crate::cpu::stark::CpuStark;
    use crate::stark::mozak_stark::{MozakStark, PublicInputs};
    use crate::stark::utils::trace_rows_to_poly_values;
    use crate::test_utils::{fast_test_config, ProveAndVerify, C, D, F};
    use crate::utils::from_u32;
    #[allow(clippy::cast_sign_loss)]
    #[test]
    fn prove_mulhsu_example() {
        type S = CpuStark<F, D>;
        let config = fast_test_config();
        let a = -2_147_451_028_i32;
        let b = 2_147_483_648_u32;
        let (program, record) = code::execute(
            [Instruction {
                op: Op::MULHSU,
                args: Args {
                    rd: 8,
                    rs1: 6,
                    rs2: 7,
                    ..Args::default()
                },
            }],
            &[],
            &[(6, a as u32), (7, b)],
        );
        let res = i64::from(a).wrapping_mul(i64::from(b));
        assert_eq!(record.executed[0].aux.dst_val, (res >> 32) as u32);
        let mut timing = TimingTree::new("mulhsu", log::Level::Debug);
        let cpu_trace = timed!(timing, "generate_cpu_trace", generate_cpu_trace(&record));
        let trace_poly_values = timed!(
            timing,
            "trace to poly",
            trace_rows_to_poly_values(cpu_trace)
        );
        let stark = S::default();
        let public_inputs = PublicInputs {
            entry_point: from_u32(program.entry_point),
        };

        let proof = timed!(
            timing,
            "cpu proof",
            prove_table::<F, C, S, D>(
                stark,
                &config,
                trace_poly_values,
                public_inputs.borrow(),
                &mut timing,
            )
        );
        let proof = proof.unwrap();
        let verification_res = timed!(
            timing,
            "cpu verification",
            verify_stark_proof(stark, proof, &config)
        );
        verification_res.unwrap();
        timing.print();
    }

    fn prove_mul<Stark: ProveAndVerify>(a: u32, b: u32) -> Result<(), TestCaseError> {
        let (program, record) = code::execute(
            [Instruction {
                op: Op::MUL,
                args: Args {
                    rd: 8,
                    rs1: 6,
                    rs2: 7,
                    ..Args::default()
                },
            }],
            &[],
            &[(6, a), (7, b)],
        );
        let low = a.wrapping_mul(b);
        prop_assert_eq!(record.executed[0].aux.dst_val, low);
        Stark::prove_and_verify(&program, &record).unwrap();
        Ok(())
    }

    fn prove_mulhu<Stark: ProveAndVerify>(a: u32, b: u32) -> Result<(), TestCaseError> {
        let (program, record) = code::execute(
            [Instruction {
                op: Op::MULHU,
                args: Args {
                    rd: 9,
                    rs1: 6,
                    rs2: 7,
                    ..Args::default()
                },
            }],
            &[],
            &[(6, a), (7, b)],
        );
        let (res, _) = u64::from(a).overflowing_mul(u64::from(b));
        prop_assert_eq!(record.executed[0].aux.dst_val, (res >> 32) as u32);
        Stark::prove_and_verify(&program, &record).unwrap();
        Ok(())
    }

    #[allow(clippy::cast_sign_loss)]
    fn prove_mulh<Stark: ProveAndVerify>(a: i32, b: i32) -> Result<(), TestCaseError> {
        let (program, record) = code::execute(
            [Instruction {
                op: Op::MULH,
                args: Args {
                    rd: 8,
                    rs1: 6,
                    rs2: 7,
                    ..Args::default()
                },
            }],
            &[],
            &[(6, a as u32), (7, b as u32)],
        );
        let (res, overflow) = i64::from(a).overflowing_mul(i64::from(b));
        assert!(!overflow);
        prop_assert_eq!(record.executed[0].aux.dst_val, (res >> 32) as u32);
        Stark::prove_and_verify(&program, &record).unwrap();
        Ok(())
    }

    #[allow(clippy::cast_sign_loss)]
    fn prove_mulhsu<Stark: ProveAndVerify>(a: i32, b: u32) -> Result<(), TestCaseError> {
        let (program, record) = code::execute(
            [Instruction {
                op: Op::MULHSU,
                args: Args {
                    rd: 8,
                    rs1: 6,
                    rs2: 7,
                    ..Args::default()
                },
            }],
            &[],
            &[(6, a as u32), (7, b)],
        );
        let (res, _overflow) = i64::from(a).overflowing_mul(i64::from(b));
        prop_assert_eq!(record.executed[0].aux.dst_val, (res >> 32) as u32);
        Stark::prove_and_verify(&program, &record).unwrap();
        Ok(())
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]
        #[test]
        fn prove_mul_cpu(a in u32_extra(), b in u32_extra()) {
            prove_mul::<CpuStark<F, D>>(a, b)?;
        }
        #[test]
        fn prove_mulhu_cpu(a in u32_extra(), b in u32_extra()) {
            prove_mulhu::<CpuStark<F, D>>(a, b)?;
        }
        #[test]
        fn prove_mulh_cpu(a in i32_extra(), b in i32_extra()) {
            prove_mulh::<CpuStark<F, D>>(a, b)?;
        }
        #[test]
        fn prove_mulhsu_cpu(a in i32_extra(), b in u32_extra()) {
            prove_mulhsu::<CpuStark<F, D>>(a, b)?;
        }

    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(1))]
        #[test]
        fn prove_mul_mozak(a in u32_extra(), b in u32_extra()) {
            prove_mul::<MozakStark<F, D>>(a, b)?;
        }
        #[test]
        fn prove_mulhu_mozak(a in u32_extra(), b in u32_extra()) {
            prove_mulhu::<MozakStark<F, D>>(a, b)?;
        }
        #[test]
        fn prove_mulh_mozak(a in i32_extra(), b in i32_extra()) {
            prove_mulh::<MozakStark<F, D>>(a, b)?;
        }
        #[test]
        fn prove_mulhsu_mozak(a in i32_extra(), b in u32_extra()) {
            prove_mulhsu::<MozakStark<F, D>>(a, b)?;
        }

    }
}

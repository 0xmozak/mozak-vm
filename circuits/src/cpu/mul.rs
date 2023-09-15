//! This module implements constraints for multiplication operations, including
//! MUL, MULH, MULHU, MULHSU and SLL instructions.
//!
//! Here, SLL stands for 'shift left logical'.  We can treat it as a variant of
//! unsigned multiplication.

use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use starky::constraint_consumer::ConstraintConsumer;

use super::bitwise::and_gadget;
use super::columns::CpuState;
use super::stark::is_binary;

/// Converts from a sign-bit to a multiplicative sign.
///
/// Specifically, if `sign_bit` is 0, returns 1.
/// And if `sign_bit` is 1, returns -1.
/// Undefined for any other input.
pub fn bit_to_sign<P: PackedField>(sign_bit: P) -> P { P::ONES - sign_bit.doubles() }

pub(crate) fn constraints<P: PackedField>(
    lv: &CpuState<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    let two_to_32 = CpuState::<P>::shifted(32);
    let op1_abs = lv.op1_abs;
    let op2_abs = lv.op2_abs;
    let low_limb = lv.product_low_limb;
    let high_limb = lv.product_high_limb;
    let product_sign = lv.product_sign;

    // Make sure product_sign is either 0 or 1.
    is_binary(yield_constr, product_sign);

    // Ensure correct computation of op1_abs * op2_abs using low_limb and high_limb.
    // If product_sign is 1, verify using the 2's complement: 2^64 - (high_limb *
    // 2^32 + low_limb).
    yield_constr.constraint(
        (P::ONES - product_sign) * (high_limb * two_to_32 + low_limb - op1_abs * op2_abs),
    );
    yield_constr.constraint(
        product_sign * (two_to_32 * (two_to_32 - high_limb) - low_limb - op1_abs * op2_abs),
    );

    // The constraints above would be enough, if our field was large enough.
    // However Goldilocks field is just a bit too small at order 2^64 - 2^32 + 1,
    // which allows for the following exploit to happen e.g. when high_limb ==
    // u32::MAX
    //
    // Specifically, (1<<32) * (u32::MAX) === -1 (mod 2^64 - 2^32 + 1).
    // Thus, when product high_limb == u32::MAX:
    //       product = low_limb + two_to_32 * high_limb =
    //       = low_limb + (1<<32) * (u32::MAX) = low_limb - P::ONES
    //
    // Which means a malicious prover could evaluate some product in two different
    // ways, which is unacceptable.
    //
    // However, the largest combined result that an honest prover can produce is
    // u32::MAX * u32::MAX = 0xFFFF_FFFE_0000_0001.  So we can add a constraint
    // that high_limb is != 0xFFFF_FFFF == u32::MAX range to prevent such exploit.

    // Make sure high_limb is not 0xFFFF_FFFF when product_sign is 0 to avoid
    // overflow.
    yield_constr.constraint(
        (P::ONES - product_sign)
            * (P::ONES
                - (P::Scalar::from_canonical_u32(0xffff_ffff) - high_limb)
                    * lv.product_high_limb_inv_helper),
    );
    // Make sure (two_to_32 - high_limb) is not 0xFFFF_FFFF when product_sign is 1
    // to avoid overflow of two_to_32 * (two_to_32 - high_limb) in the above
    // constraints.
    yield_constr.constraint(product_sign * (P::ONES - high_limb * lv.product_high_limb_inv_helper));

    // Make sure op1_abs is computed correctly from op1_value.
    yield_constr.constraint(op1_abs - lv.op1_full_range() * bit_to_sign(lv.op1_sign_bit));

    // Make sure op2_abs is computed correctly from op2_value for MUL operations.
    // Note that for SLL, op2_abs is computed from bitshift.multiplier.
    yield_constr.constraint(
        (P::ONES - lv.inst.ops.sll)
            * (op2_abs - lv.op2_full_range() * bit_to_sign(lv.op2_sign_bit)),
    );

    // For MUL/MULHU/SLL product sign should always be 0.
    yield_constr.constraint((lv.inst.ops.sll + lv.inst.ops.mul + lv.inst.ops.mulhu) * product_sign);

    // Ensure skip_check_product_sign can be set to 1 only when either ob1_abs or
    // op2_abs is 0. This check is essential for the subsequent constraints.
    // We are not concerned with other values of skip_check_product_sign.
    yield_constr.constraint(lv.skip_check_product_sign * lv.op1_abs * lv.op2_abs);

    // Make sure product_sign is computed correctly.
    yield_constr.constraint(
        (P::ONES - lv.skip_check_product_sign)
            * (bit_to_sign(product_sign)
                - bit_to_sign(lv.op1_sign_bit) * bit_to_sign(lv.op2_sign_bit)),
    );

    // Check: for SLL the multiplier is assigned as `2^(op2 & 0b1_111)`.
    // We only take lowest 5 bits of the op2 for the shift amount.
    // This is following the RISC-V specification.
    // Below we use the And gadget to calculate the shift amount, and then use
    // Bitshift table to retrieve the corresponding power of 2, that we will assign
    // to the multiplier.
    {
        let and_gadget = and_gadget(&lv.xor);
        yield_constr.constraint(
            lv.inst.ops.sll * (and_gadget.input_a - P::Scalar::from_noncanonical_u64(0b1_1111)),
        );
        let op2 = lv.op2_value;
        yield_constr.constraint(lv.inst.ops.sll * (and_gadget.input_b - op2));

        yield_constr.constraint(lv.inst.ops.sll * (and_gadget.output - lv.bitshift.amount));
        yield_constr.constraint(lv.inst.ops.sll * (op2_abs - lv.bitshift.multiplier));
    }

    // Now, check, that we select the correct output based on the opcode.
    let destination = lv.dst_value;
    yield_constr.constraint((lv.inst.ops.mul + lv.inst.ops.sll) * (destination - low_limb));
    yield_constr.constraint(
        (lv.inst.ops.mulh + lv.inst.ops.mulhsu + lv.inst.ops.mulhu) * (destination - high_limb),
    );
}

#[cfg(test)]
#[allow(clippy::cast_possible_wrap)]
mod tests {
    use anyhow::Result;
    use mozak_executor::instruction::{Args, Instruction, Op};
    use mozak_executor::test_utils::{i32_extra, reg, simple_test_code, u32_extra};
    use plonky2::timed;
    use plonky2::util::timing::TimingTree;
    use proptest::prelude::{prop_assume, ProptestConfig};
    use proptest::test_runner::TestCaseError;
    use proptest::{prop_assert_eq, proptest};
    use starky::prover::prove as prove_table;
    use starky::verifier::verify_stark_proof;

    use crate::cpu::stark::CpuStark;
    use crate::generation::cpu::{generate_cpu_trace, generate_cpu_trace_extended};
    use crate::generation::program::generate_program_rom_trace;
    use crate::stark::mozak_stark::{MozakStark, PublicInputs};
    use crate::stark::utils::trace_to_poly_values;
    use crate::test_utils::{standard_faster_config, ProveAndVerify, C, D, F};
    use crate::utils::from_u32;
    #[allow(clippy::cast_sign_loss)]
    #[allow(clippy::cast_lossless)]
    #[test]
    fn prove_mulhsu_example() {
        type S = CpuStark<F, D>;
        let config = standard_faster_config();
        let a = -2_147_451_028_i32;
        let b = 2_147_483_648_u32;
        let (program, record) = simple_test_code(
            &[Instruction {
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
        let cpu_trace = timed!(
            timing,
            "generate_cpu_trace",
            generate_cpu_trace(&program, &record)
        );
        let trace_poly_values = timed!(
            timing,
            "trace to poly",
            trace_to_poly_values(generate_cpu_trace_extended(
                cpu_trace,
                &generate_program_rom_trace(&program)
            ))
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
                public_inputs.into(),
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
        let (program, record) = simple_test_code(
            &[Instruction {
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
        let (low, _high) = a.widening_mul(b);
        prop_assert_eq!(record.executed[0].aux.dst_val, low);
        Stark::prove_and_verify(&program, &record).unwrap();
        Ok(())
    }

    fn prove_mulhu<Stark: ProveAndVerify>(a: u32, b: u32) -> Result<(), TestCaseError> {
        let (program, record) = simple_test_code(
            &[Instruction {
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
        let (_low, high) = a.widening_mul(b);
        prop_assert_eq!(record.executed[0].aux.dst_val, high);
        Stark::prove_and_verify(&program, &record).unwrap();
        Ok(())
    }

    #[allow(clippy::cast_sign_loss)]
    #[allow(clippy::cast_lossless)]
    fn prove_mulh<Stark: ProveAndVerify>(a: i32, b: i32) -> Result<(), TestCaseError> {
        let (program, record) = simple_test_code(
            &[Instruction {
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
    #[allow(clippy::cast_lossless)]
    fn prove_mulhsu<Stark: ProveAndVerify>(a: i32, b: u32) -> Result<(), TestCaseError> {
        let (program, record) = simple_test_code(
            &[Instruction {
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

    fn prove_sll<Stark: ProveAndVerify>(
        p: u32,
        q: u32,
        rs1: u8,
        rs2: u8,
        rd: u8,
    ) -> Result<(), TestCaseError> {
        prop_assume!(rs1 != rs2);
        prop_assume!(rs1 != rd);
        prop_assume!(rs2 != rd);
        let (program, record) = simple_test_code(
            &[
                Instruction {
                    op: Op::SLL,
                    args: Args {
                        rd,
                        rs1,
                        rs2,
                        ..Args::default()
                    },
                },
                Instruction {
                    op: Op::SLL,
                    args: Args {
                        rd,
                        rs1,
                        imm: q,
                        ..Args::default()
                    },
                },
            ],
            &[],
            &[(rs1, p), (rs2, q)],
        );
        prop_assert_eq!(record.executed[0].aux.dst_val, p << (q & 0b1_1111));
        prop_assert_eq!(record.executed[1].aux.dst_val, p << (q & 0b1_1111));
        Stark::prove_and_verify(&program, &record).unwrap();
        Ok(())
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(4))]
        #[test]
        fn prove_mul_cpu(a in u32_extra(), b in u32_extra()) {
            prove_mul::<CpuStark<F, D>>(a, b)?;
        }
        #[test]
        fn prove_mulhu_cpu(a in u32_extra(), b in u32_extra()) {
            prove_mulhu::<CpuStark<F, D>>(a, b)?;
        }
        #[allow(clippy::cast_sign_loss)]
        #[allow(clippy::cast_lossless)]
        #[test]
        fn prove_mulh_cpu(a in i32_extra(), b in i32_extra()) {
            prove_mulh::<CpuStark<F, D>>(a, b)?;
        }
        #[allow(clippy::cast_sign_loss)]
        #[allow(clippy::cast_lossless)]
        #[test]
        fn prove_mulhsu_cpu(a in i32_extra(), b in u32_extra()) {
            prove_mulhsu::<CpuStark<F, D>>(a, b)?;
        }

        #[test]
        fn prove_sll_cpu(p in u32_extra(), q in u32_extra(), rs1 in reg(), rs2 in reg(), rd in reg()) {
            prove_sll::<CpuStark<F, D>>(p, q, rs1, rs2, rd)?;
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(4))]
        #[test]
        fn prove_mul_mozak(a in u32_extra(), b in u32_extra()) {
            prove_mul::<MozakStark<F, D>>(a, b)?;
        }
        #[test]
        fn prove_mulhu_mozak(a in u32_extra(), b in u32_extra()) {
            prove_mulhu::<MozakStark<F, D>>(a, b)?;
        }
        #[allow(clippy::cast_sign_loss)]
        #[allow(clippy::cast_lossless)]
        #[test]
        fn prove_mulh_mozak(a in i32_extra(), b in i32_extra()) {
            prove_mulh::<MozakStark<F, D>>(a, b)?;
        }
        #[allow(clippy::cast_sign_loss)]
        #[allow(clippy::cast_lossless)]
        #[test]
        fn prove_mulhsu_mozak(a in i32_extra(), b in u32_extra()) {
            prove_mulhsu::<MozakStark<F, D>>(a, b)?;
        }

        #[test]
        fn prove_sll_mozak(p in u32_extra(), q in u32_extra(), rs1 in reg(), rs2 in reg(), rd in reg()) {
            prove_sll::<MozakStark<F, D>>(p, q, rs1, rs2, rd)?;
        }
    }
}

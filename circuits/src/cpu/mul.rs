//! This module implements constraints for multiplication operations, including
//! MUL, MULH, MULHU, MULHSU and SLL instructions.
//!
//! Here, SLL stands for 'shift left logical'.  We can treat it as a variant of
//! unsigned multiplication.

use plonky2::field::extension::Extendable;
use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};

use super::columns::{op1_full_range_extension_target, op2_full_range_extension_target, CpuState};
use crate::stark::utils::{is_binary, is_binary_ext_circuit};

/// Converts from a sign-bit to a multiplicative sign.
///
/// Specifically, if `sign_bit` is 0, returns 1.
/// And if `sign_bit` is 1, returns -1.
/// Undefined for any other input.
pub fn bit_to_sign<P: PackedField>(sign_bit: P) -> P { P::ONES - sign_bit.doubles() }

pub(crate) fn bit_to_sign_extension<F: RichField + Extendable<D>, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    sign_bit: ExtensionTarget<D>,
) -> ExtensionTarget<D> {
    let ones = builder.one_extension();
    let sign_bit_doubled = builder.add_extension(sign_bit, sign_bit);
    builder.sub_extension(ones, sign_bit_doubled)
}

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
    yield_constr.constraint(op2_abs - lv.op2_full_range() * bit_to_sign(lv.op2_sign_bit));

    // If both factors are unsigned, the output will always be
    // non-negative/unsigned. As an optimization, we take advantage of the fact
    // that is_op1_signed == 0 implies is_op2_signed == 0 for all our operations.
    // (In fact, the two values only differ for MULHSU.)
    yield_constr.constraint((P::ONES - lv.inst.is_op1_signed) * product_sign);

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

    // Now, check, that we select the correct output based on the opcode.
    let destination = lv.dst_value;
    yield_constr.constraint((lv.inst.ops.mul + lv.inst.ops.sll) * (destination - low_limb));
    yield_constr.constraint((lv.inst.ops.mulh) * (destination - high_limb));
}

pub(crate) fn constraints_circuit<F: RichField + Extendable<D>, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    lv: &CpuState<ExtensionTarget<D>>,
    yield_constr: &mut RecursiveConstraintConsumer<F, D>,
) {
    let two_to_32 = builder.constant_extension(F::Extension::from_canonical_u64(1 << 32));
    let op1_abs = lv.op1_abs;
    let op2_abs = lv.op2_abs;
    let low_limb = lv.product_low_limb;
    let high_limb = lv.product_high_limb;
    let product_sign = lv.product_sign;

    is_binary_ext_circuit(builder, product_sign, yield_constr);

    let one = builder.one_extension();
    let high_limb_mul_two_to_32 = builder.mul_extension(high_limb, two_to_32);
    let high_limb_mul_two_to_32_add_low_limb =
        builder.add_extension(high_limb_mul_two_to_32, low_limb);
    let op1_abs_mul_op2_abs = builder.mul_extension(op1_abs, op2_abs);
    let one_sub_product_sign = builder.sub_extension(one, product_sign);
    let temp1 = builder.sub_extension(high_limb_mul_two_to_32_add_low_limb, op1_abs_mul_op2_abs);
    let first_constraint = builder.mul_extension(one_sub_product_sign, temp1);
    yield_constr.constraint(builder, first_constraint);

    let two_to_32_sub_high_limb = builder.sub_extension(two_to_32, high_limb);
    let two_to_32_mul_two_to_32_sub_high_limb =
        builder.mul_extension(two_to_32, two_to_32_sub_high_limb);
    let temp2 = builder.sub_extension(two_to_32_mul_two_to_32_sub_high_limb, low_limb);
    let temp3 = builder.sub_extension(temp2, op1_abs_mul_op2_abs);
    let second_constraint = builder.mul_extension(product_sign, temp3);
    yield_constr.constraint(builder, second_constraint);

    let one_sub_product_sign = builder.sub_extension(one, product_sign);
    let max_u32 = builder.constant_extension(F::Extension::from_canonical_u32(u32::MAX));
    let max_u32_sub_high_limb = builder.sub_extension(max_u32, high_limb);
    let max_u32_sub_high_limb_mul_product_high_limb_inv_helper =
        builder.mul_extension(max_u32_sub_high_limb, lv.product_high_limb_inv_helper);
    let one_sub_max_u32_sub_high_limb_mul_product_high_limb_inv_helper =
        builder.sub_extension(one, max_u32_sub_high_limb_mul_product_high_limb_inv_helper);
    let third_constraint = builder.mul_extension(
        one_sub_product_sign,
        one_sub_max_u32_sub_high_limb_mul_product_high_limb_inv_helper,
    );
    yield_constr.constraint(builder, third_constraint);

    let high_limb_mul_product_high_limb_inv_helper =
        builder.mul_extension(high_limb, lv.product_high_limb_inv_helper);
    let one_sub_high_limb_mul_product_high_limb_inv_helper =
        builder.sub_extension(one, high_limb_mul_product_high_limb_inv_helper);
    let fourth_constraint = builder.mul_extension(
        product_sign,
        one_sub_high_limb_mul_product_high_limb_inv_helper,
    );
    yield_constr.constraint(builder, fourth_constraint);

    let op1_full_range = op1_full_range_extension_target(builder, lv);
    let bit_to_sign_op1_sign_bit = bit_to_sign_extension(builder, lv.op1_sign_bit);
    let op1_full_range_mul_bit_to_sign_op1_sign_bit =
        builder.mul_extension(op1_full_range, bit_to_sign_op1_sign_bit);
    let fifth_constraint =
        builder.sub_extension(op1_abs, op1_full_range_mul_bit_to_sign_op1_sign_bit);
    yield_constr.constraint(builder, fifth_constraint);

    let op2_full_range = op2_full_range_extension_target(builder, lv);
    let bit_to_sign_op2_sign_bit = bit_to_sign_extension(builder, lv.op2_sign_bit);
    let op2_full_range_mul_bit_to_sign_op2_sign_bit =
        builder.mul_extension(op2_full_range, bit_to_sign_op2_sign_bit);
    let sixth_constraint =
        builder.sub_extension(op2_abs, op2_full_range_mul_bit_to_sign_op2_sign_bit);
    yield_constr.constraint(builder, sixth_constraint);

    let one_sub_is_op1_signed = builder.sub_extension(one, lv.inst.is_op1_signed);
    let seventh_constraint = builder.mul_extension(one_sub_is_op1_signed, product_sign);
    yield_constr.constraint(builder, seventh_constraint);

    let skip_check_product_sign_mul_op1_abs =
        builder.mul_extension(lv.skip_check_product_sign, op1_abs);
    let eighth_constraint = builder.mul_extension(skip_check_product_sign_mul_op1_abs, op2_abs);
    yield_constr.constraint(builder, eighth_constraint);

    let one_sub_skip_check_product_sign = builder.sub_extension(one, lv.skip_check_product_sign);
    let bit_to_sign_product_sign = bit_to_sign_extension(builder, product_sign);
    let bit_to_sign_op1_sign_bit_mul_bit_to_sign_op2_sign_bit =
        builder.mul_extension(bit_to_sign_op1_sign_bit, bit_to_sign_op2_sign_bit);
    let ninth_constraint = builder.sub_extension(
        bit_to_sign_product_sign,
        bit_to_sign_op1_sign_bit_mul_bit_to_sign_op2_sign_bit,
    );
    let ninth_constraint = builder.mul_extension(one_sub_skip_check_product_sign, ninth_constraint);
    yield_constr.constraint(builder, ninth_constraint);

    let destination = lv.dst_value;
    let mul_add_sll = builder.add_extension(lv.inst.ops.mul, lv.inst.ops.sll);
    let destination_sub_low_limb = builder.sub_extension(destination, low_limb);
    let tenth_constraint = builder.mul_extension(mul_add_sll, destination_sub_low_limb);
    yield_constr.constraint(builder, tenth_constraint);
    let destination_sub_high_limb = builder.sub_extension(destination, high_limb);
    let eleventh_constraint = builder.mul_extension(lv.inst.ops.mulh, destination_sub_high_limb);
    yield_constr.constraint(builder, eleventh_constraint);
}

#[cfg(test)]
#[allow(clippy::cast_possible_wrap)]
mod tests {

    use std::borrow::Borrow;

    use anyhow::Result;
    use mozak_runner::instruction::{Args, Instruction, Op};
    use mozak_runner::test_utils::{i32_extra, u32_extra};
    use mozak_runner::util::execute_code;
    use plonky2::timed;
    use plonky2::util::timing::TimingTree;
    use proptest::prelude::ProptestConfig;
    use proptest::test_runner::TestCaseError;
    use proptest::{prop_assert_eq, proptest};
    use starky::prover::prove as prove_table;
    use starky::verifier::verify_stark_proof;

    use crate::cpu::stark::CpuStark;
    use crate::generation::cpu::{generate_cpu_trace, generate_cpu_trace_extended};
    use crate::generation::program::generate_program_rom_trace;
    use crate::stark::mozak_stark::{MozakStark, PublicInputs};
    use crate::stark::utils::trace_to_poly_values;
    use crate::test_utils::{fast_test_config, ProveAndVerify, C, D, F};
    use crate::utils::from_u32;
    #[allow(clippy::cast_sign_loss)]
    #[allow(clippy::cast_lossless)]
    #[test]
    fn prove_mulhsu_example() {
        type S = CpuStark<F, D>;
        let config = fast_test_config();
        let a = -2_147_451_028_i32;
        let b = 2_147_483_648_u32;
        let (program, record) = execute_code(
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
        let (program, record) = execute_code(
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
        let (program, record) = execute_code(
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
    #[allow(clippy::cast_lossless)]
    fn prove_mulh<Stark: ProveAndVerify>(a: i32, b: i32) -> Result<(), TestCaseError> {
        let (program, record) = execute_code(
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
    #[allow(clippy::cast_lossless)]
    fn prove_mulhsu<Stark: ProveAndVerify>(a: i32, b: u32) -> Result<(), TestCaseError> {
        let (program, record) = execute_code(
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

    }
}

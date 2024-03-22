//! This module implements constraints for division operations, including
//! DIVU, REMU, DIV, REM, SRL and SRA instructions.
//!
//! Here, SRL stands for 'shift right logical'.  We can treat it as a variant of
//! unsigned division. Same for SRA.

use plonky2::field::extension::Extendable;
use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};

use super::columns::{op1_full_range_extension_target, op2_full_range_extension_target, CpuState};
use crate::cpu::mul::{bit_to_sign, bit_to_sign_extension};
use crate::stark::utils::{is_binary, is_binary_ext_circuit};

/// Constraints for DIV / REM / DIVU / REMU / SRL / SRA instructions
pub(crate) fn constraints<P: PackedField>(
    lv: &CpuState<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    let ops = lv.inst.ops;
    let two_to_32 = CpuState::<P>::shifted(32);
    let dividend_value = lv.op1_value;
    let dividend_sign = lv.op1_sign_bit;
    let dividend_abs = lv.op1_abs;
    let dividend_full_range = lv.op1_full_range();
    let divisor_value = lv.op2_value;
    let divisor_sign = lv.op2_sign_bit;
    let divisor_abs = lv.op2_abs;
    let divisor_full_range = lv.op2_full_range();

    // The following columns are used only in this function, which requires extra
    // checks or range checks.
    let divisor_value_inv = lv.op2_value_inv;
    let quotient_value = lv.quotient_value;
    let quotient_sign = lv.quotient_sign;
    let remainder_value = lv.remainder_value;
    let remainder_sign = lv.remainder_sign;
    let remainder_slack = lv.remainder_slack;
    let quotient_full_range = quotient_value - quotient_sign * two_to_32; // Equation (1)
    let remainder_full_range = remainder_value - remainder_sign * two_to_32;
    let quotient_abs = bit_to_sign(quotient_sign) * quotient_full_range;
    let remainder_abs = bit_to_sign(remainder_sign) * remainder_full_range;

    // For both signed and unsigned division, it holds that
    // |dividend| = |divisor| × |quotient| + |remainder|.
    // Note that for SRA the remainder is always non-negative, so when dividend < 0
    // this equation becomes |dividend| = |divisor| × |quotient| - remainder.
    yield_constr.constraint(
        divisor_abs * quotient_abs
            + (P::ONES - ops.sra) * remainder_abs
            + ops.sra * (bit_to_sign(dividend_sign) * remainder_full_range)
            - dividend_abs,
    );

    // We also need to make sure quotient_sign and remainder_sign are set correctly.
    is_binary(yield_constr, remainder_sign);
    is_binary(yield_constr, dividend_sign);
    yield_constr
        .constraint((P::ONES - ops.sra) * remainder_value * (dividend_sign - remainder_sign));
    yield_constr.constraint(ops.sra * remainder_sign);

    // Quotient_sign = dividend_sign * divisor_sign, with three exceptions:
    // 1. When divisor = 0, this case is handled below.
    // 2. When quotient = 0, we do not care about the sign.
    // 3. For signed instructions, when quotient = 2^31 (overflow), quotient_sign is
    //    not important.
    yield_constr.constraint(
        (P::ONES - lv.skip_check_quotient_sign)
            * (bit_to_sign(quotient_sign) - bit_to_sign(dividend_sign) * bit_to_sign(divisor_sign)),
    );
    // Ensure that 'skip_check_quotient_sign' can only be set to 1 in the presence
    // of the above exceptions. For other potential values, it does not
    // matter and will not break any constraints.
    // 'skip_check_quotient_sign' is introduced to keep the constraints low-degree.
    //
    // Some notes about 'quotient_value + quotient_full_range':
    // According to equation (1), it can be written as:
    //     quotient_value * 2 = quotient_sign * two_to_32
    // 1. When quotient = 0, quotient_value = quotient_full_range = 0.
    // 2. When quotient = 2^31 (overflow case for quotient_sign = 1), quotient_value
    //    = 2^31, quotient_full_range = -2^31.
    // For a range-checked quotient_value, a malicious prover cannot set this
    // expression to 0 with any other values.
    yield_constr.constraint(
        lv.skip_check_quotient_sign * divisor_full_range * (quotient_value + quotient_full_range),
    );

    // https://five-embeddev.com/riscv-isa-manual/latest/m.html says
    // > For both signed and unsigned division, it holds that
    // > dividend = divisor × quotient + remainder.
    yield_constr.constraint(
        (P::ONES - lv.skip_check_quotient_sign)
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
    yield_constr
        .constraint(divisor_abs * (remainder_abs + P::ONES + remainder_slack - divisor_abs));

    // Constraints for divisor == 0.  On RISC-V:
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
    yield_constr.constraint((ops.div + ops.srl + ops.sra) * (dst - quotient_value));
    yield_constr.constraint(ops.rem * (dst - remainder_value));
}

#[allow(clippy::too_many_lines)]
pub(crate) fn constraints_circuit<F: RichField + Extendable<D>, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    lv: &CpuState<ExtensionTarget<D>>,
    yield_constr: &mut RecursiveConstraintConsumer<F, D>,
) {
    let ops = lv.inst.ops;
    let two_to_32 = builder.constant_extension(F::Extension::from_canonical_u64(1 << 32));
    let dividend_value = lv.op1_value;
    let dividend_sign = lv.op1_sign_bit;
    let dividend_abs = lv.op1_abs;
    let dividend_full_range = op1_full_range_extension_target(builder, lv);
    let divisor_value = lv.op2_value;
    let divisor_sign = lv.op2_sign_bit;
    let divisor_abs = lv.op2_abs;
    let divisor_full_range = op2_full_range_extension_target(builder, lv);

    let divisor_value_inv = lv.op2_value_inv;
    let quotient_value = lv.quotient_value;
    let quotient_sign = lv.quotient_sign;
    let remainder_value = lv.remainder_value;
    let remainder_sign = lv.remainder_sign;
    let remainder_slack = lv.remainder_slack;

    let quotient_sign_mul_two_to_32 = builder.mul_extension(quotient_sign, two_to_32);
    let quotient_full_range = builder.sub_extension(quotient_value, quotient_sign_mul_two_to_32);
    let remainder_sign_mul_two_to_32 = builder.mul_extension(remainder_sign, two_to_32);
    let remainder_full_range = builder.sub_extension(remainder_value, remainder_sign_mul_two_to_32);
    let bit_to_sign_quotient_sign = bit_to_sign_extension(builder, quotient_sign);
    let quotient_abs = builder.mul_extension(bit_to_sign_quotient_sign, quotient_full_range);
    let bit_to_sign_remainder_sign = bit_to_sign_extension(builder, remainder_sign);
    let remainder_abs = builder.mul_extension(bit_to_sign_remainder_sign, remainder_full_range);

    let one = builder.one_extension();
    let divisor_abs_mul_quotient_abs = builder.mul_extension(divisor_abs, quotient_abs);
    let one_sub_ops_sra = builder.sub_extension(one, ops.sra);
    let one_sub_ops_sra_mul_remainder_abs = builder.mul_extension(one_sub_ops_sra, remainder_abs);
    let bit_to_sign_dividend_sign = bit_to_sign_extension(builder, dividend_sign);
    let sra_mul_bit_to_sign_dividend_sign =
        builder.mul_extension(ops.sra, bit_to_sign_dividend_sign);
    let sra_mul_bit_to_sign_dividend_sign_mul_remainder_full_range =
        builder.mul_extension(sra_mul_bit_to_sign_dividend_sign, remainder_full_range);
    let constr = builder.add_extension(
        divisor_abs_mul_quotient_abs,
        one_sub_ops_sra_mul_remainder_abs,
    );
    let constr = builder.add_extension(
        constr,
        sra_mul_bit_to_sign_dividend_sign_mul_remainder_full_range,
    );
    let constr = builder.sub_extension(constr, dividend_abs);
    yield_constr.constraint(builder, constr);

    is_binary_ext_circuit(builder, remainder_sign, yield_constr);
    is_binary_ext_circuit(builder, dividend_sign, yield_constr);

    let dividend_sign_sub_remainder_sign = builder.sub_extension(dividend_sign, remainder_sign);
    let one_sub_ops_sra_mul_remainder_value =
        builder.mul_extension(one_sub_ops_sra, remainder_value);
    let constr = builder.mul_extension(
        one_sub_ops_sra_mul_remainder_value,
        dividend_sign_sub_remainder_sign,
    );
    yield_constr.constraint(builder, constr);
    let ops_sra_mul_remainder_sign = builder.mul_extension(ops.sra, remainder_sign);
    yield_constr.constraint(builder, ops_sra_mul_remainder_sign);

    let bit_to_sign_divisor_sign = bit_to_sign_extension(builder, divisor_sign);
    let bit_to_sign_dividend_sign_mul_bit_to_sign_divisor_sign =
        builder.mul_extension(bit_to_sign_dividend_sign, bit_to_sign_divisor_sign);
    let bit_to_sign_quotient_sign_sub = builder.sub_extension(
        bit_to_sign_quotient_sign,
        bit_to_sign_dividend_sign_mul_bit_to_sign_divisor_sign,
    );
    let one_sub_skip_check_quotient_sign = builder.sub_extension(one, lv.skip_check_quotient_sign);
    let constr = builder.mul_extension(
        one_sub_skip_check_quotient_sign,
        bit_to_sign_quotient_sign_sub,
    );
    yield_constr.constraint(builder, constr);

    let skip_check_quotient_sign_mul_divisor_full_range =
        builder.mul_extension(lv.skip_check_quotient_sign, divisor_full_range);
    let quotient_value_add_quotient_full_range =
        builder.add_extension(quotient_value, quotient_full_range);
    let constr = builder.mul_extension(
        skip_check_quotient_sign_mul_divisor_full_range,
        quotient_value_add_quotient_full_range,
    );
    yield_constr.constraint(builder, constr);

    let divisor_full_range_mul_quotient_full_range =
        builder.mul_extension(divisor_full_range, quotient_full_range);
    let constr = builder.add_extension(
        divisor_full_range_mul_quotient_full_range,
        remainder_full_range,
    );
    let constr = builder.sub_extension(constr, dividend_full_range);
    let constr = builder.mul_extension(one_sub_skip_check_quotient_sign, constr);
    yield_constr.constraint(builder, constr);

    let remainder_abs_add_one = builder.add_extension(remainder_abs, one);
    let remainder_abs_add_one_add_remainder_slack =
        builder.add_extension(remainder_abs_add_one, remainder_slack);
    let constr = builder.sub_extension(remainder_abs_add_one_add_remainder_slack, divisor_abs);
    let constr = builder.mul_extension(divisor_abs, constr);
    yield_constr.constraint(builder, constr);

    let divisor_value_mul_divisor_value_inv =
        builder.mul_extension(divisor_value, divisor_value_inv);
    let one_sub_divisor_value_mul_divisor_value_inv =
        builder.sub_extension(one, divisor_value_mul_divisor_value_inv);
    let u32_max = builder.constant_extension(F::Extension::from_canonical_u32(u32::MAX));
    let quotient_value_sub_max = builder.sub_extension(quotient_value, u32_max);
    let constr = builder.mul_extension(
        one_sub_divisor_value_mul_divisor_value_inv,
        quotient_value_sub_max,
    );
    yield_constr.constraint(builder, constr);
    let remainder_value_sub_dividend_value = builder.sub_extension(remainder_value, dividend_value);
    let constr = builder.mul_extension(
        one_sub_divisor_value_mul_divisor_value_inv,
        remainder_value_sub_dividend_value,
    );
    yield_constr.constraint(builder, constr);

    let dst = lv.dst_value;
    let div_add_srl = builder.add_extension(ops.div, ops.srl);
    let ops_div_srl_sra = builder.add_extension(div_add_srl, ops.sra);
    let dst_sub_quotient_value = builder.sub_extension(dst, quotient_value);
    let constr = builder.mul_extension(ops_div_srl_sra, dst_sub_quotient_value);
    yield_constr.constraint(builder, constr);
    let dst_sub_remainder_value = builder.sub_extension(dst, remainder_value);
    let ops_rem_mul_dst_sub_remainder_value =
        builder.mul_extension(ops.rem, dst_sub_remainder_value);
    yield_constr.constraint(builder, ops_rem_mul_dst_sub_remainder_value);
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use mozak_runner::instruction::{Args, Instruction, Op};
    use mozak_runner::test_utils::u32_extra;
    use mozak_runner::util::execute_code;
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
        let (program, record) = execute_code(divu_remu_instructions(rd), &[], &[(1, p), (2, q)]);
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

    fn prove_div<Stark: ProveAndVerify>(p: u32, q: u32, rd: u8) {
        let (program, record) = execute_code(div_rem_instructions(rd), &[], &[(1, p), (2, q)]);
        Stark::prove_and_verify(&program, &record).unwrap();
    }

    #[allow(clippy::cast_sign_loss)]
    #[test]
    fn prove_div_example() { prove_div::<CpuStark<F, D>>(i32::MIN as u32, -1_i32 as u32, 28); }

    #[allow(clippy::cast_sign_loss)]
    #[test]
    fn prove_div_mozak_example() {
        prove_div::<MozakStark<F, D>>(i32::MIN as u32, -1_i32 as u32, 28);
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]
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
            prove_div::<CpuStark<F, D>>(p, q, rd);
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
            prove_div::<MozakStark<F, D>>(p, q, rd);
        }

        #[test]
        fn prove_divu_mozak(p in u32_extra(), q in u32_extra(), rd in 3_u8..32) {
            prove_divu::<MozakStark<F, D>>(p, q, rd)?;
        }
    }
}

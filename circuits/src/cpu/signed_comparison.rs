//! This module implements constraints for comparisons, SLT and SLTU.
//! Where `SLT` means 'Set if Less Then', and 'SLTU' is the same but unsigned.

use plonky2::field::extension::Extendable;
use plonky2::field::packed::PackedField;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};

use super::columns::CpuState;
use crate::stark::utils::{is_binary, is_binary_ext_circuit};

/// # Explanation
///
/// `opX_full_range` is the value of an operand opX as if converted to i64.
/// For unsigned operations: `Field::from_noncanonical_i64(opX as i64)`
/// For signed operations: `Field::from_noncanonical_i64(opX as i32 as i64)`
///
/// Expressed in terms of field elements it is:
/// ```ignore
/// opX_full_range = opX_value - self.opX_sign_bit * (1 << 32)
/// ```
//
/// Our constraints need to ensure, that the prover did this conversion
/// properly. For an unsigned operation, the range of `opX_full_range` is
/// `0..=u32::MAX`. For an signed operation, the range of `opX_full_range` is
/// `i32::MIN..=i32::MAX`. Notice how both ranges are of the same length, and
/// only differ by an offset of `1<<31`.

pub(crate) fn signed_constraints<P: PackedField>(
    lv: &CpuState<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    is_binary(yield_constr, lv.op1_sign_bit);
    is_binary(yield_constr, lv.op2_sign_bit);
    // When op1 is not signed as per instruction semantics, op1_sign_bit must be 0.
    yield_constr.constraint((P::ONES - lv.inst.is_op1_signed) * lv.op1_sign_bit);
    // When op2 is not signed as per instruction semantics, op2_sign_bit must be 0.
    yield_constr.constraint((P::ONES - lv.inst.is_op2_signed) * lv.op2_sign_bit);
}

pub(crate) fn signed_constraints_circuit<F: RichField + Extendable<D>, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    lv: &CpuState<ExtensionTarget<D>>,
    yield_constr: &mut RecursiveConstraintConsumer<F, D>,
) {
    is_binary_ext_circuit(builder, lv.op1_sign_bit, yield_constr);
    is_binary_ext_circuit(builder, lv.op2_sign_bit, yield_constr);

    let one = builder.one_extension();
    let one_sub_is_op1_signed = builder.sub_extension(one, lv.inst.is_op1_signed);
    let op1_sign_bit_constr = builder.mul_extension(one_sub_is_op1_signed, lv.op1_sign_bit);
    yield_constr.constraint(builder, op1_sign_bit_constr);

    let one_sub_is_op2_signed = builder.sub_extension(one, lv.inst.is_op2_signed);
    let op2_sign_bit_constr = builder.mul_extension(one_sub_is_op2_signed, lv.op2_sign_bit);
    yield_constr.constraint(builder, op2_sign_bit_constr);
}

pub(crate) fn slt_constraints<P: PackedField>(
    lv: &CpuState<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    // Check: the destination has the same value as stored in `less_than`.
    yield_constr.constraint(lv.inst.ops.slt * (lv.less_than - lv.dst_value));
}

pub(crate) fn slt_constraints_circuit<F: RichField + Extendable<D>, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    lv: &CpuState<ExtensionTarget<D>>,
    yield_constr: &mut RecursiveConstraintConsumer<F, D>,
) {
    let less_than_sub_dst_value = builder.sub_extension(lv.less_than, lv.dst_value);
    let slt_constr = builder.mul_extension(lv.inst.ops.slt, less_than_sub_dst_value);

    yield_constr.constraint(builder, slt_constr);
}

#[cfg(test)]
#[allow(clippy::cast_possible_wrap)]
mod tests {
    use mozak_runner::instruction::{Args, Instruction, Op};
    use mozak_runner::test_utils::{execute_code, u32_extra};
    use proptest::prelude::{any, ProptestConfig};
    use proptest::proptest;

    use crate::cpu::stark::CpuStark;
    use crate::stark::mozak_stark::MozakStark;
    use crate::test_utils::{ProveAndVerify, D, F};

    fn prove_slt<Stark: ProveAndVerify>(a: u32, op2: u32, use_imm: bool) {
        let (b, imm) = if use_imm { (0, op2) } else { (op2, 0) };
        let (program, record) = execute_code(
            [
                Instruction {
                    op: Op::SLTU,
                    args: Args {
                        rd: 5,
                        rs1: 6,
                        rs2: 7,
                        imm,
                    },
                },
                Instruction {
                    op: Op::SLT,
                    args: Args {
                        rd: 4,
                        rs1: 6,
                        rs2: 7,
                        imm,
                    },
                },
            ],
            &[],
            &[(6, a), (7, b)],
        );
        assert_eq!(record.last_state.get_register_value(5), u32::from(a < op2));
        assert_eq!(
            record.last_state.get_register_value(4),
            u32::from((a as i32) < (op2 as i32))
        );
        Stark::prove_and_verify(&program, &record).unwrap();
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(4))]
        #[test]
        fn prove_slt_cpu(a in u32_extra(), op2 in u32_extra(), use_imm in any::<bool>()) {
            prove_slt::<CpuStark<F, D>>(a, op2, use_imm);
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(1))]
        #[test]
        fn prove_slt_mozak(a in u32_extra(), op2 in u32_extra(), use_imm in any::<bool>()) {
            prove_slt::<MozakStark<F, D>>(a, op2, use_imm);
        }
    }
}

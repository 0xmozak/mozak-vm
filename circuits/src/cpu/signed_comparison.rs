//! This module implements constraints for comparisons, SLT and SLTU.
//! Where `SLT` means 'Set if Less Then', and 'SLTU' is the same but unsigned.

use plonky2::field::packed::PackedField;
use starky::constraint_consumer::ConstraintConsumer;

use super::columns::CpuState;
use crate::cpu::stark::is_binary;

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
/// `0..=u32::MAX`. For an unsigned operation, the range of `opX_full_range` is
/// `i32::MIN..=i32::MAX`. Notice how both ranges are of the same length, and
/// only differ by an offset of `1<<31`.
///
/// TODO: range check these two linear combinations of columns:
/// ```ignore
///  lv.op1_full_range() + lv.is_signed() * CpuState::<P>::shifted(31);
///  lv.op2_full_range() + lv.is_signed() * CpuState::<P>::shifted(31);
/// ```

pub(crate) fn signed_constraints<P: PackedField>(
    lv: &CpuState<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    is_binary(yield_constr, lv.op1_sign_bit);
    is_binary(yield_constr, lv.op2_sign_bit);
}

pub(crate) fn slt_constraints<P: PackedField>(
    lv: &CpuState<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    // Check: the destination has the same value as stored in `less_than`.
    yield_constr.constraint((lv.inst.ops.slt + lv.inst.ops.sltu) * (lv.less_than - lv.dst_value));
}

#[cfg(test)]
#[allow(clippy::cast_possible_wrap)]
mod tests {
    use mozak_vm::instruction::{Args, Instruction, Op};
    use mozak_vm::test_utils::{simple_test_code, u32_extra};
    use proptest::prelude::{any, ProptestConfig};
    use proptest::proptest;

    use crate::cpu::stark::CpuStark;
    use crate::test_utils::ProveAndVerify;
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(4))]
        #[test]
        fn prove_slt_proptest(a in u32_extra(), op2 in u32_extra(), use_imm in any::<bool>()) {
            let (b, imm) = if use_imm { (0, op2) } else { (op2, 0) };
            let (program, record) = simple_test_code(
                &[
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
            CpuStark::prove_and_verify(&program, &record).unwrap();
        }
    }
}

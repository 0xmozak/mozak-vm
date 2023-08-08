use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use starky::constraint_consumer::ConstraintConsumer;

use super::columns::CpuColumnsView;

/// Constraints for SLTU / SLT instructions
///
/// SLT stands for 'shift less than'.  
/// It assigns 1 to the result if rs1 value is less than rs2.
/// It assigns 0 otherwise.
pub(crate) fn constraints<P: PackedField>(
    lv: &CpuColumnsView<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    let p32 = P::Scalar::from_noncanonical_u64(1 << 32);
    let p31 = P::Scalar::from_noncanonical_u64(1 << 31);

    let is_cmp = lv.inst.ops.slt + lv.inst.ops.sltu;

    // Check: domain of `less_than`, `op1_sign`, and `op2_sign` is {0, 1}.

    let lt = lv.less_than;
    yield_constr.constraint(lt * (P::ONES - lt));

    // (`+`, `-`) are represented by (0, 1) respectively.
    let sign1 = lv.op1_sign;
    yield_constr.constraint(sign1 * (P::ONES - sign1));
    let sign2 = lv.op2_sign;
    yield_constr.constraint(sign2 * (P::ONES - sign2));

    let op1 = lv.op1_value;
    let op2 = lv.op2_value;

    // TODO: range check
    let op1_fixed = lv.op1_val_fixed;
    // TODO: range check
    let op2_fixed = lv.op2_val_fixed;

    // Check: values have been fixed correctly.
    // Fixing implies representing the value as lexicographically increasing format.
    // Fixing signed values is done by adding 2^31 to the value.
    // This turns i32::MIN representation into 0, and i32::MAX into 2^32 - 1.
    // Fixing unsigned values does not do anything to the value.
    yield_constr.constraint(lv.inst.ops.sltu * (op1_fixed - op1));
    yield_constr.constraint(lv.inst.ops.sltu * (op2_fixed - op2));

    yield_constr.constraint(lv.inst.ops.slt * (op1_fixed - (op1 + p31 - sign1 * p32)));
    yield_constr.constraint(lv.inst.ops.slt * (op2_fixed - (op2 + p31 - sign2 * p32)));

    let diff_fixed = op1_fixed - op2_fixed;
    // TODO: range check
    let abs_diff = lv.cmp_abs_diff;

    // Check: sign of the diff_fixed maps to the the output value.
    // When diff_fixed is positive, op1 > op2, and result is 0.
    // When diff_fixed is negative, op1 < op2, and result is 1.
    // Note that these two checks is insufficient for case when diff == 0.
    yield_constr.constraint(is_cmp * (P::ONES - lt) * (abs_diff - diff_fixed));
    yield_constr.constraint(is_cmp * lt * (abs_diff + diff_fixed));

    // Check: when diff is 0, result is 0.
    let diff = op1 - op2;
    let diff_inv = lv.cmp_diff_inv;
    yield_constr.constraint(lt * (P::ONES - diff * diff_inv));

    // Check: result is copied out correctly.
    yield_constr.constraint(is_cmp * (lt - lv.dst_value));
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
                            ..Args::default()
                        },
                    },
                    Instruction {
                        op: Op::SLT,
                        args: Args {
                            rd: 4,
                            rs1: 6,
                            rs2: 7,
                            imm,
                            ..Args::default()
                        },
                    },
                ],
                &[],
                &[(6, a), (7, b)],
            );
            assert_eq!(record.last_state.get_register_value(5), (a < op2).into());
            assert_eq!(
                record.last_state.get_register_value(4),
                ((a as i32) < (op2 as i32)).into()
            );
            CpuStark::prove_and_verify(&program, &record.executed).unwrap();
        }
    }
}

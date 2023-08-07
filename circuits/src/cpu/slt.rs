use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use starky::constraint_consumer::ConstraintConsumer;

use super::columns::CpuColumnsView;

pub(crate) fn constraints<P: PackedField>(
    lv: &CpuColumnsView<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    let p31 = P::Scalar::from_noncanonical_u64(1 << 31);
    let is_signed1_op = lv.inst.ops.ops_that_has_signed_op1().into_iter().sum::<P>();
    let is_signed2_op = lv.inst.ops.ops_that_has_signed_op2().into_iter().sum::<P>();

    let is_cmp = lv.inst.ops.slt + lv.inst.ops.sltu;

    let lt = lv.less_than;
    yield_constr.constraint(lt * (P::ONES - lt));

    let op1 = lv.op1_value;
    let op2 = lv.op2_value;
    // TODO: range check
    let op1_fixed = lv.op1_sign_adjusted - p31 * is_signed1_op;
    // TODO: range check
    let op2_fixed = lv.op2_sign_adjusted - p31 * is_signed2_op;

    let diff_fixed = op1_fixed - op2_fixed;
    // TODO: range check
    let abs_diff = lv.cmp_abs_diff;

    // abs_diff calculation
    yield_constr.constraint(is_cmp * (P::ONES - lt) * (abs_diff - diff_fixed));
    yield_constr.constraint(is_cmp * lt * (abs_diff + diff_fixed));

    let diff = op1 - op2;
    let diff_inv = lv.cmp_diff_inv;
    yield_constr.constraint(lt * (P::ONES - diff * diff_inv));
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

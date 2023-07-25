use plonky2::field::packed::PackedField;
use starky::constraint_consumer::ConstraintConsumer;

use super::columns::CpuColumnsView;

pub(crate) fn signed_constraints<P: PackedField>(
    lv: &CpuColumnsView<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    let p32 = CpuColumnsView::<P>::p32();
    let p31 = CpuColumnsView::<P>::p31();
    let is_signed = lv.is_signed();

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

    yield_constr.constraint(op1_fixed - (op1 + is_signed * p31 - sign1 * p32));
    yield_constr.constraint(op2_fixed - (op2 + is_signed * p31 - sign2 * p32));
}

pub(crate) fn slt_constraints<P: PackedField>(
    lv: &CpuColumnsView<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    yield_constr.constraint((lv.ops.slt + lv.ops.sltu) * (lv.less_than - lv.dst_value));
}

#[cfg(test)]
#[allow(clippy::cast_possible_wrap)]
mod tests {
    use mozak_vm::instruction::{Args, Instruction, Op};
    use mozak_vm::test_utils::{simple_test_code, u32_extra};
    use proptest::prelude::{any, ProptestConfig};
    use proptest::proptest;

    use crate::test_utils::simple_proof_test;
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(4))]
        #[test]
        fn prove_slt_proptest(a in u32_extra(), op2 in u32_extra(), use_imm in any::<bool>()) {
            let (b, imm) = if use_imm { (0, op2) } else { (op2, 0) };
            let record = simple_test_code(
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
            simple_proof_test(&record.executed).unwrap();
        }
    }
}

use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use starky::constraint_consumer::ConstraintConsumer;

use super::columns::CpuState;

pub(crate) fn constraints<P: PackedField>(
    lv: &CpuState<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    let p32 = P::Scalar::from_noncanonical_u64(1 << 32);
    let p31 = P::Scalar::from_noncanonical_u64(1 << 31);

    let is_cmp = lv.inst.ops.slt + lv.inst.ops.sltu;

    let lt = lv.less_than;
    yield_constr.constraint(lt * (P::ONES - lt));

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

    yield_constr.constraint(lv.inst.ops.sltu * (op1_fixed - op1));
    yield_constr.constraint(lv.inst.ops.sltu * (op2_fixed - op2));

    yield_constr.constraint(lv.inst.ops.slt * (op1_fixed - (op1 + p31 - sign1 * p32)));
    yield_constr.constraint(lv.inst.ops.slt * (op2_fixed - (op2 + p31 - sign2 * p32)));

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

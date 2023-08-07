use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use starky::constraint_consumer::ConstraintConsumer;

use super::columns::CpuColumnsView;

pub(crate) fn constraints<P: PackedField>(
    lv: &CpuColumnsView<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    let expected_value = lv.op1_value - lv.op2_value;
    let wrapped = P::Scalar::from_noncanonical_u64(1 << 32) + expected_value;
    yield_constr
        .constraint(lv.inst.ops.sub * ((lv.dst_value - expected_value) * (lv.dst_value - wrapped)));
}

#[cfg(test)]
#[allow(clippy::cast_possible_wrap)]
mod tests {
    use mozak_vm::instruction::{Args, Instruction, Op};
    use mozak_vm::test_utils::{simple_test_code, u32_extra};
    use proptest::prelude::ProptestConfig;
    use proptest::proptest;

    use crate::cpu::stark::CpuStark;
    use crate::test_utils::ProveAndVerify;
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(4))]
        #[test]
        fn prove_sub_proptest(a in u32_extra(), b in u32_extra()) {
            let record = simple_test_code(
                &[Instruction {
                    op: Op::SUB,
                    args: Args {
                        rd: 5,
                        rs1: 6,
                        rs2: 7,
                        ..Args::default()
                    },
                }],
                &[],
                &[(6, a), (7, b)],
            );
            assert_eq!(record.last_state.get_register_value(5), a.wrapping_sub(b));
            CpuStark::prove_and_verify(&record.executed).unwrap();
        }
    }
}

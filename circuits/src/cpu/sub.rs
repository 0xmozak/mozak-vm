use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use starky::constraint_consumer::ConstraintConsumer;

use super::columns::{COL_DST_VALUE, COL_OP1_VALUE, COL_OP2_VALUE, COL_S_SUB, NUM_CPU_COLS};

pub(crate) fn constraints<P: PackedField>(
    lv: &[P; NUM_CPU_COLS],
    yield_constr: &mut ConstraintConsumer<P>,
) {
    let expected_value = lv[COL_OP1_VALUE] - lv[COL_OP2_VALUE];
    let wrapped = P::Scalar::from_noncanonical_u64(1 << 32) + expected_value;
    yield_constr.constraint(
        lv[COL_S_SUB] * ((lv[COL_DST_VALUE] - expected_value) * (lv[COL_DST_VALUE] - wrapped)),
    );
}

#[cfg(test)]
#[allow(clippy::cast_possible_wrap)]
mod tests {
    use mozak_vm::instruction::{Args, Instruction, Op};
    use mozak_vm::test_utils::{simple_test_code, u32_extra};
    use proptest::prelude::ProptestConfig;
    use proptest::proptest;

    use crate::test_utils::simple_proof_test;
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
            simple_proof_test(&record.executed).unwrap();
        }
    }
}

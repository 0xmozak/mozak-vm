use plonky2::field::packed::PackedField;
use starky::constraint_consumer::ConstraintConsumer;

use super::columns::{
    COL_DST_VALUE, COL_OP1_VALUE, COL_OP2_VALUE, COL_PC, COL_S_SUB, NUM_CPU_COLS,
};
use crate::utils::column_of_xs;

pub(crate) fn constraints<P: PackedField>(
    lv: &[P; NUM_CPU_COLS],
    nv: &[P; NUM_CPU_COLS],
    yield_constr: &mut ConstraintConsumer<P>,
) {
    let expected_value = lv[COL_OP1_VALUE] - lv[COL_OP2_VALUE];
    let wrapped = column_of_xs::<P>(1 << 32) + expected_value;
    yield_constr.constraint(
        lv[COL_S_SUB] * ((lv[COL_DST_VALUE] - expected_value) * (lv[COL_DST_VALUE] - wrapped)),
    );

    // pc ticks up
    let inc: P = column_of_xs(4);
    yield_constr.constraint_transition((lv[COL_S_SUB]) * (nv[COL_PC] - lv[COL_PC] - inc));
}

#[cfg(test)]
#[allow(clippy::cast_possible_wrap)]
mod test {
    use mozak_vm::instruction::{Args, Instruction, Op};
    use mozak_vm::test_utils::simple_test_code;

    use crate::test_utils::simple_proof_test;
    use proptest::prelude::any;
    use proptest::proptest;
    proptest! {
        #[test]
        fn prove_sub_proptest(a in any::<u32>(), b in any::<u32>()) {
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

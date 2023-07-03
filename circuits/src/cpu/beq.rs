use plonky2::field::packed::PackedField;
use starky::constraint_consumer::ConstraintConsumer;

use super::columns::{
    COL_CMP_ABS_DIFF, COL_CMP_DIFF_INV, COL_DST_VALUE, COL_IMM_VALUE, COL_LESS_THAN, COL_OP1_VALUE,
    COL_OP2_VALUE, COL_S_SLT, COL_S_SLTU, COL_S_SLT_OP1_VAL_FIXED, COL_S_SLT_OP2_VAL_FIXED,
    COL_S_SLT_SIGN1, COL_S_SLT_SIGN2, NUM_CPU_COLS,
};
use crate::utils::column_of_xs;

pub(crate) fn constraints<P: PackedField>(
    lv: &[P; NUM_CPU_COLS],
    yield_constr: &mut ConstraintConsumer<P>,
) {

    let is_beq = lv[COL_S_BEQ];



    let op1 = lv[COL_OP1_VALUE];
    let op2 = lv[COL_OP2_VALUE];

    let diff = op1 - op2;
    // TODO: range check
    let abs_diff = lv[COL_CMP_ABS_DIFF];

    // abs_diff calculation
    yield_constr.constraint(is_beq * (abs_diff - diff));

    let diff = op1 - op2;
    let diff_inv = lv[COL_CMP_DIFF_INV];
    let branch =  diff * diff_inv;
    // either branch is 0 or 1
    yield_constr.constraint(branch * (P::ONES - branch)); 
    yield_constr.constraint(is_beq * (((P::ONES - branch) * (lv[COL_DST_VALUE] - lv[COL_IMM_VALUE]))  + (branch *(lv[COL_DST_VALUE] - (lv[COL_PC] + column_of_xs(4))))));
}

#[cfg(test)]
#[allow(clippy::cast_possible_wrap)]
mod test {
    use mozak_vm::instruction::{Args, Instruction, Op};
    use mozak_vm::test_utils::simple_test_code;
    use proptest::prelude::any;
    use proptest::proptest;

    use crate::test_utils::simple_proof_test;
    proptest! {
            #[test]
            fn prove_beq_proptest(a in any::<u32>(), b in any::<u32>()) {
                let record = simple_test_code(
                    &[Instruction {
                        op: Op::BEQ,
                        args: Args {
                            rd: 0,
                            rs1: 6,
                            rs2: 7,
                            imm: 8,
                        },
                    },
                    // if above branch is not taken R1 has value 10.
                    Instruction {
                        op: Op::ADD,
                        args: Args {
                            rd: 1,
                            rs1: 0,
                            rs2: 0,
                            imm: 10,
                        },
                    }
                    ],
                    &[],
                    &[(6, a), (7, b)],
                );
                    if a != b {
                        assert_eq!(record.last_state.get_register_value(1), 10);
                    }
                simple_proof_test(&record.executed).unwrap();
            }
    }
}

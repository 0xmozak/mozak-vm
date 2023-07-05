use plonky2::field::packed::PackedField;
use starky::constraint_consumer::ConstraintConsumer;

use super::columns::{
    COL_CMP_DIFF_INV, COL_EQUAL, COL_IMM_VALUE, COL_OP1_VALUE, COL_OP2_VALUE, COL_PC, COL_S_BEQ,
    NUM_CPU_COLS,
};
use crate::utils::column_of_xs;

pub(crate) fn constraints<P: PackedField>(
    lv: &[P; NUM_CPU_COLS],
    nv: &[P; NUM_CPU_COLS],
    yield_constr: &mut ConstraintConsumer<P>,
) {
    let is_beq = lv[COL_S_BEQ];

    let op1 = lv[COL_OP1_VALUE];
    let op2 = lv[COL_OP2_VALUE];

    let diff = op1 - op2;
    let diff_inv = lv[COL_CMP_DIFF_INV];
    let branch = diff * diff_inv;
    let equal = lv[COL_EQUAL];
    yield_constr.constraint(is_beq * (branch - (P::ONES - equal)));
    let updated_pc = nv[COL_PC];
    yield_constr.constraint(equal * (P::ONES - branch));
    let pc_next: P = (P::ONES - equal) * (updated_pc - (lv[COL_PC] + column_of_xs::<P>(4)));
    let pc_branch: P = equal * (updated_pc - lv[COL_IMM_VALUE]);
    let pc_cons: P = pc_next + pc_branch;
    yield_constr.constraint(is_beq * pc_cons);
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
                simple_proof_test(&record.executed).unwrap();
            }
    }
}

use plonky2::field::packed::PackedField;
use starky::constraint_consumer::ConstraintConsumer;

use super::columns::{
    COL_DST_VALUE, COL_IMM_VALUE, COL_OP1_VALUE, COL_PC, COL_S_JALR, NUM_CPU_COLS,
};
use crate::utils::column_of_xs;

pub(crate) fn constraints<P: PackedField>(
    lv: &[P; NUM_CPU_COLS],
    nv: &[P; NUM_CPU_COLS],
    yield_constr: &mut ConstraintConsumer<P>,
) {
    let wrap_at: P = column_of_xs(1 << 32);

    let return_address = lv[COL_PC] + column_of_xs::<P>(4);
    let wrapped_return_address = return_address - wrap_at;

    // enable-if JALR: aux.dst_val == jmp-inst-pc + 4, wrapped
    yield_constr.constraint(
        lv[COL_S_JALR]
            * (lv[COL_DST_VALUE] - return_address)
            * (lv[COL_DST_VALUE] - wrapped_return_address),
    );

    let jump_address = lv[COL_PC] + lv[COL_IMM_VALUE] + lv[COL_OP1_VALUE];
    let wrapped_jump_address = jump_address - wrap_at;

    // enable-of JALR
    yield_constr.constraint_transition(
        lv[COL_S_JALR] * (nv[COL_PC] - jump_address) * (nv[COL_PC] - wrapped_jump_address),
    );
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
        fn prove_jalr_goto_no_rs1_proptest(a in any::<u32>(), b in any::<u32>()) {
            let record = simple_test_code(
                &[Instruction {
                    op: Op::JALR,
                    args: Args {
                        rd: 0,
                        rs1: 0,
                        imm: 4,
                        ..Args::default()
                    },
                }],
                &[],
                &[(0x1,a),(0x2,b)],
            );
            assert_eq!(record.last_state.get_pc(), 8);
            simple_proof_test(&record.executed).unwrap();
        }
        #[test]
        fn prove_jalr_goto_rs1_zero_proptest(a in any::<u32>(), b in any::<u32>()) {
            let record = simple_test_code(
                &[Instruction {
                    op: Op::JALR,
                    args: Args {
                        rd: 0,
                        rs1: 1,
                        imm: 4,
                        ..Args::default()
                    },
                }],
                &[],
                &[(0x1,0),(0x2,a),(0x3,b)],
            );
            assert_eq!(record.last_state.get_pc(), 8);
            simple_proof_test(&record.executed).unwrap();
        }

        #[test]
        fn prove_jalr_goto_imm_zero_rs1_not_zero_proptest(a in any::<u32>(), b in any::<u32>()) {
            let record = simple_test_code(
                &[Instruction {
                    op: Op::JALR,
                    args: Args {
                        rd: 0,
                        rs1: 1,
                        imm: 0,
                        ..Args::default()
                    },
                }],
                &[],
                &[(0x1,4),(0x2,a),(0x3,b)],
            );
            assert_eq!(record.last_state.get_pc(), 8);
            simple_proof_test(&record.executed).unwrap();
        }

        #[test]
        fn prove_jalr_proptest(a in any::<u32>(), b in any::<u32>()) {
            let record = simple_test_code(
                &[Instruction {
                    op: Op::JALR,
                    args: Args {
                        rd: 1,
                        rs1: 0,
                        imm: 4,
                        ..Args::default()
                    },
                }],
                &[],
                &[(0x1, 0), (0x2,a),(0x3,b)],
            );
            assert_eq!(record.last_state.get_pc(), 8);
            simple_proof_test(&record.executed).unwrap();
        }
        /*
        #[test]
        fn prove_double_jalr_proptest(a in any::<u32>(), b in any::<u32>()) {
            let record = simple_test_code(
                &[
                    Instruction {
                        op: Op::JALR,
                        args: Args {
                            rd: 1,  // return address in x1 will be pc + 4
                            rs1: 0,
                            imm: 4, // jump to next instruction
                            ..Args::default()
                        },
                    },
                    Instruction {
                        op: Op::ADD,
                        args: Args {
                            rd: 1,  // return address in x1 will be pc + 4
                            rs1: 0,
                            imm: 4, // jump to next instruction
                            ..Args::default()
                        },
                    },
                    Instruction {
                        op: Op::JALR,
                        args: Args {
                            rd: 1,  // return address in x1 will be pc + 4
                            rs1: 0,
                            imm: 4, // jump to next instruction
                            ..Args::default()
                        },
                }],
                &[],
                &[(0x1, 0), (0x2,a),(0x3,b)],
            );
            // assert_eq!(record.last_state.get_register_value(5), a.wrapping_sub(b));
            assert_eq!(record.last_state.get_pc(), 8);
            simple_proof_test(&record.executed).unwrap();
        }

         */
    }
}

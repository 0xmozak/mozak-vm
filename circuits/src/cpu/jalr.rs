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

    yield_constr.constraint_transition(
        lv[COL_S_JALR] * (nv[COL_PC] - jump_address) * (nv[COL_PC] - wrapped_jump_address),
    );
}
#[cfg(test)]
#[allow(clippy::cast_possible_wrap)]
mod test {
    use mozak_vm::instruction::{Args, Instruction, Op};
    use mozak_vm::test_utils::simple_test_code;
    use mozak_vm::vm::Row;
    use proptest::prelude::any;
    use proptest::proptest;

    use crate::test_utils::simple_proof_test;

    #[test]
    fn prove_jalr_goto_no_rs1() {
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
            &[],
        );
        assert_eq!(record.last_state.get_pc(), 8);
        simple_proof_test(&record.executed).unwrap();
    }

    #[test]
    fn prove_jalr_goto_rs1_zero() {
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
            &[(0x1, 0)],
        );
        assert_eq!(record.last_state.get_pc(), 8);
        simple_proof_test(&record.executed).unwrap();
    }
    #[test]
    fn prove_jalr_goto_imm_zero_rs1_not_zero() {
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
            &[(0x1, 4)],
        );
        assert_eq!(record.last_state.get_pc(), 8);
        simple_proof_test(&record.executed).unwrap();
    }

    #[test]
    fn prove_jalr() {
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
            &[(0x1, 0)],
        );
        assert_eq!(record.last_state.get_pc(), 8);
        simple_proof_test(&record.executed).unwrap();
    }

    #[test]
    fn prove_double_jalr() {
        let record = simple_test_code(
            &[
                Instruction {
                    op: Op::JALR,
                    args: Args {
                        imm: 8, // goto to pc + 4
                        ..Args::default()
                    },
                },
                Instruction {
                    op: Op::JALR,
                    args: Args {
                        imm: 12, // goto to pc + 4
                        ..Args::default()
                    },
                },
                Instruction {
                    op: Op::JALR,
                    args: Args {
                        imm: 4, // goto to pc + 4
                        ..Args::default()
                    },
                },
            ],
            &[],
            &[],
        );
        assert_eq!(record.last_state.get_pc(), 16);
        simple_proof_test(&record.executed).unwrap();
    }

    proptest! {
        #[test]
        fn jalr_jumps_past_an_instruction(rs1 in 1_u8..32, rs1_val in any::<u32>(), rd in 1_u8..32, sentinel in any::<u32>()) {
            let jump_target: u32 = 8;
            let imm = jump_target.wrapping_sub(rs1_val);
            let record = simple_test_code(
                &[Instruction {
                    op: Op::JALR,
                    args: Args {
                        rd,
                        rs1,
                        imm,
                        ..Args::default()
                    },
                },
                // We are jumping past this instruction, so it should not be executed.
                // So we should not overwrite register `rd` with `sentinel`.
                Instruction {
                    op: Op::ADD,
                    args: Args {
                        rd,
                        imm: sentinel,
                        ..Args::default()
                    },
                }],
                &[],
                &[(rs1, rs1_val)],
            );
            assert_eq!(record.executed.len(), 3);
            // simple_test_code adds a simple coda to the end of the program to ensure it halts.
            // We are interested in the state just before entering the coda.
            let [.., Row {state, ..}, _] = &record.executed[..]
                else { unreachable!() };
            assert_eq!(state.get_register_value(rd), 4);
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

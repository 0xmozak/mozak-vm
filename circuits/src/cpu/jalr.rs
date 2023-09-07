//! This module implements the JALR operation constraints
//! JALR writes the address of the instruction following the jump, being pc + 4,
//! And then sets the target address with sum of signed immediate and rs1.

use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use starky::constraint_consumer::ConstraintConsumer;

use super::columns::CpuState;

pub(crate) fn constraints<P: PackedField>(
    lv: &CpuState<P>,
    nv: &CpuState<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    let wrap_at = P::Scalar::from_noncanonical_u64(1 << 32);

    // Save the address of the instruction following the jump (return address).
    let return_address = lv.inst.pc + P::Scalar::from_noncanonical_u64(4);
    let wrapped_return_address = return_address - wrap_at;

    let destination = lv.dst_value;
    // Check: the wrapped `pc + 4` is saved to destination.
    // As values are u32 range checked, this makes the value choice deterministic.
    yield_constr.constraint(
        lv.inst.ops.jalr * (destination - return_address) * (destination - wrapped_return_address),
    );

    let jump_target = lv.op1_value + lv.op2_value;
    let wrapped_jump_target = jump_target - wrap_at;
    let new_pc = nv.inst.pc;

    // Check: the wrapped op1, op2 sum is set as new `pc`.
    // As values are u32 range checked, this makes the value choice deterministic.
    yield_constr.constraint_transition(
        lv.inst.ops.jalr * (new_pc - jump_target) * (new_pc - wrapped_jump_target),
    );
}

#[cfg(test)]
mod tests {
    use mozak_vm::instruction::{Args, Instruction, Op};
    use mozak_vm::test_utils::{reg, simple_test_code, state_before_final, u32_extra};
    use proptest::prelude::ProptestConfig;
    use proptest::proptest;

    use crate::cpu::stark::CpuStark;
    use crate::stark::mozak_stark::MozakStark;
    use crate::test_utils::{ProveAndVerify, D, F};

    #[test]
    fn prove_jalr_goto_no_rs1() {
        let (program, record) = simple_test_code(
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
        CpuStark::prove_and_verify(&program, &record).unwrap();
    }

    #[test]
    fn prove_jalr_goto_rs1_zero() {
        let (program, record) = simple_test_code(
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
        CpuStark::prove_and_verify(&program, &record).unwrap();
    }

    #[test]
    fn prove_jalr_goto_imm_zero_rs1_not_zero() {
        let (program, record) = simple_test_code(
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
        CpuStark::prove_and_verify(&program, &record).unwrap();
    }

    #[test]
    fn prove_jalr() {
        let (program, record) = simple_test_code(
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
        CpuStark::prove_and_verify(&program, &record).unwrap();
    }

    fn prove_triple_jalr<Stark: ProveAndVerify>() {
        let (program, record) = simple_test_code(
            &[
                Instruction {
                    op: Op::JALR,
                    args: Args {
                        imm: 8, // goto to pc = 8
                        ..Args::default()
                    },
                },
                Instruction {
                    op: Op::JALR,
                    args: Args {
                        imm: 12, // goto to pc = 12
                        ..Args::default()
                    },
                },
                Instruction {
                    op: Op::JALR,
                    args: Args {
                        imm: 4, // goto to pc = 4
                        ..Args::default()
                    },
                },
            ],
            &[],
            &[],
        );
        assert_eq!(record.last_state.get_pc(), 16);
        Stark::prove_and_verify(&program, &record).unwrap();
    }

    #[test]
    fn prove_triple_jalr_cpu() { prove_triple_jalr::<CpuStark<F, D>>() }

    #[test]
    fn prove_triple_jalr_mozak() { prove_triple_jalr::<MozakStark<F, D>>() }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(4))]
        #[test]
        fn jalr_jumps_past_an_instruction(rs1 in reg(), rs1_val in u32_extra(), rd in reg(), sentinel in u32_extra()) {
            let jump_target: u32 = 8;
            let imm = jump_target.wrapping_sub(rs1_val);
            let (program, record) = simple_test_code(
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
            assert_eq!(state_before_final(&record).get_register_value(rd), 4);
            CpuStark::prove_and_verify(&program, &record).unwrap();
        }
    }
}

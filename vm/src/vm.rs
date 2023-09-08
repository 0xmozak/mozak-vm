use anyhow::Result;

use crate::elf::Program;
use crate::instruction::{Args, Op};
use crate::state::{Aux, State};
use crate::system::ecall;
use crate::system::reg_abi::REG_A0;

#[must_use]
#[allow(clippy::cast_sign_loss)]
#[allow(clippy::cast_possible_wrap)]
pub fn mulh(a: u32, b: u32) -> u32 { ((i64::from(a as i32) * i64::from(b as i32)) >> 32) as u32 }

#[must_use]
pub fn mulhu(a: u32, b: u32) -> u32 { ((u64::from(a) * u64::from(b)) >> 32) as u32 }

#[must_use]
#[allow(clippy::cast_sign_loss)]
#[allow(clippy::cast_possible_wrap)]
pub fn mulhsu(a: u32, b: u32) -> u32 { ((i64::from(a as i32) * i64::from(b)) >> 32) as u32 }

#[must_use]
#[allow(clippy::cast_sign_loss)]
#[allow(clippy::cast_possible_wrap)]
pub fn div(a: u32, b: u32) -> u32 {
    match (a as i32, b as i32) {
        (_, 0) => 0xFFFF_FFFF,
        (a, b) => a.overflowing_div(b).0 as u32,
    }
}

#[must_use]
pub fn divu(a: u32, b: u32) -> u32 {
    match (a, b) {
        (_, 0) => 0xFFFF_FFFF,
        (a, b) => a / b,
    }
}

#[must_use]
#[allow(clippy::cast_sign_loss)]
#[allow(clippy::cast_possible_wrap)]
pub fn rem(a: u32, b: u32) -> u32 {
    (match (a as i32, b as i32) {
        (a, 0) => a,
        // overflow when -2^31 / -1
        (-0x8000_0000, -1) => 0,
        (a, b) => a % b,
    }) as u32
}

#[must_use]
pub fn remu(a: u32, b: u32) -> u32 {
    match (a, b) {
        (a, 0) => a,
        (a, b) => a % b,
    }
}

#[must_use]
#[allow(clippy::cast_sign_loss)]
#[allow(clippy::cast_possible_wrap)]
pub fn lb(mem: &[u8; 4]) -> u32 { i32::from(mem[0] as i8) as u32 }

#[must_use]
pub fn lbu(mem: &[u8; 4]) -> u32 { mem[0].into() }

#[must_use]
#[allow(clippy::cast_sign_loss)]
#[allow(clippy::cast_possible_wrap)]
pub fn lh(mem: &[u8; 4]) -> u32 { i32::from(i16::from_le_bytes([mem[0], mem[1]])) as u32 }

#[must_use]
pub fn lhu(mem: &[u8; 4]) -> u32 { u16::from_le_bytes([mem[0], mem[1]]).into() }

#[must_use]
pub fn lw(mem: &[u8; 4]) -> u32 { u32::from_le_bytes(*mem) }

impl State {
    #[must_use]
    pub fn jalr(self, inst: &Args) -> (Aux, Self) {
        let new_pc = self.get_register_value(inst.rs1).wrapping_add(inst.imm) & !1;
        let dst_val = self.get_pc().wrapping_add(4);
        (
            Aux {
                dst_val,
                ..Default::default()
            },
            self.set_pc(new_pc).set_register_value(inst.rd, dst_val),
        )
    }

    #[must_use]
    pub fn ecall(self) -> (Aux, Self) {
        match self.get_register_value(REG_A0) {
            ecall::HALT => {
                // Note: we don't advance the program counter for 'halt'.
                // That is we treat 'halt' like an endless loop.
                (
                    Aux {
                        will_halt: true,
                        ..Aux::default()
                    },
                    self.halt(),
                )
            }
            _ => (Aux::default(), self.bump_pc()),
        }
    }

    #[must_use]
    pub fn store(self, inst: &Args, bytes: u32) -> (Aux, Self) {
        let dst_val: u32 = self.get_register_value(inst.rs1);
        let addr = self.get_register_value(inst.rs2).wrapping_add(inst.imm);
        (
            Aux {
                dst_val,
                mem_addr: Some(addr),
                ..Default::default()
            },
            (0..bytes)
                .map(|i| addr.wrapping_add(i))
                .zip(dst_val.to_le_bytes())
                .fold(self, |acc, (i, byte)| acc.store_u8(i, byte))
                .bump_pc(),
        )
    }

    #[must_use]
    #[allow(clippy::cast_sign_loss)]
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_possible_wrap)]
    pub fn execute_instruction(self, program: &Program) -> (Aux, Self) {
        let inst = self.current_instruction(program);
        macro_rules! rop {
            ($op: expr) => {
                self.register_op(&inst.args, $op)
            };
        }
        // TODO: consider factoring out this logic into trace generation.
        let rs1 = self.get_register_value(inst.args.rs1);
        let rs2 = self.get_register_value(inst.args.rs2);
        let op1 = if matches!(inst.op, Op::DIV | Op::REM) {
            div(rs1, rs2)
        } else if matches!(inst.op, Op::DIVU | Op::REMU) {
            divu(rs1, rs2)
        } else if inst.op == Op::SRL {
            rs1 >> (rs2 & 0b1_1111)
        } else {
            rs1
        };
        // For branch and div instructions, both op2 and imm serve different purposes.
        // Therefore, we avoid adding them together here.
        let op2 = if matches!(
            inst.op,
            Op::BEQ | Op::BNE | Op::BLT | Op::BLTU | Op::BGE | Op::BGEU
        ) {
            rs2
        } else {
            rs2.wrapping_add(inst.args.imm)
        };

        let (aux, state) = match inst.op {
            Op::ADD => rop!(u32::wrapping_add),
            // Only use lower 5 bits of rs2 or imm
            Op::SLL => rop!(|a, b| a << (b & 0b1_1111)),
            // Only use lower 5 bits of rs2 or imm
            Op::SRL => rop!(|a, b| a >> (b & 0b1_1111)),
            // Only use lower 5 bits of rs2 or imm
            Op::SRA => rop!(|a, b| (a as i32 >> (b & 0b1_1111) as i32) as u32),
            Op::SLT => rop!(|a, b| u32::from((a as i32) < (b as i32))),
            Op::SLTU => rop!(|a, b| u32::from(a < b)),
            Op::AND => rop!(core::ops::BitAnd::bitand),
            Op::OR => rop!(core::ops::BitOr::bitor),
            Op::XOR => rop!(core::ops::BitXor::bitxor),
            Op::SUB => rop!(u32::wrapping_sub),

            Op::LB => self.memory_load(&inst.args, lb),
            Op::LBU => self.memory_load(&inst.args, lbu),
            Op::LH => self.memory_load(&inst.args, lh),
            Op::LHU => self.memory_load(&inst.args, lhu),
            Op::LW => self.memory_load(&inst.args, lw),

            Op::ECALL => self.ecall(),
            Op::JALR => self.jalr(&inst.args),
            // branches
            Op::BEQ => self.branch_op(&inst.args, |a, b| a == b),
            Op::BNE => self.branch_op(&inst.args, |a, b| a != b),
            Op::BLT => self.branch_op(&inst.args, |a, b| (a as i32) < (b as i32)),
            Op::BLTU => self.branch_op(&inst.args, |a, b| a < b),
            Op::BGE => self.branch_op(&inst.args, |a, b| (a as i32) >= (b as i32)),
            Op::BGEU => self.branch_op(&inst.args, |a, b| a >= b),
            // branching done.
            Op::SW => self.store(&inst.args, 4),
            Op::SH => self.store(&inst.args, 2),
            Op::SB => self.store(&inst.args, 1),
            Op::MUL => rop!(u32::wrapping_mul),
            Op::MULH => rop!(mulh),
            Op::MULHU => rop!(mulhu),
            Op::MULHSU => rop!(mulhsu),
            Op::DIV => rop!(div),
            Op::DIVU => rop!(divu),
            Op::REM => rop!(rem),
            Op::REMU => rop!(remu),
            Op::UNKNOWN => unimplemented!("Unknown instruction"),
        };
        (
            Aux {
                new_pc: state.get_pc(),
                op1,
                op2,
                ..aux
            },
            state.bump_clock(),
        )
    }
}

/// Each row corresponds to the state of the VM _just before_ executing the
/// instruction that the program counter points to.
#[derive(Debug, Clone, Default)]
pub struct Row {
    pub state: State,
    pub aux: Aux,
}

#[derive(Debug, Default)]
pub struct ExecutionRecord {
    pub executed: Vec<Row>,
    pub last_state: State,
}

/// Execute a program
///
/// # Errors
/// This function returns an error, if an instruction could not be loaded
/// or executed.
///
/// # Panics
/// Panics in debug mode, when executing more steps than specified in
/// environment variable `MOZAK_MAX_LOOPS` at compile time.  Defaults to one
/// million steps.
/// This is a temporary measure to catch problems with accidental infinite
/// loops. (Matthias had some trouble debugging a problem with jumps
/// earlier.)
pub fn step(program: &Program, mut last_state: State) -> Result<ExecutionRecord> {
    let mut executed = vec![];
    while !last_state.has_halted() {
        let (aux, new_state) = last_state.clone().execute_instruction(program);
        executed.push(Row {
            state: last_state,
            aux,
        });
        last_state = new_state;

        if cfg!(debug_assertions) {
            let limit: u64 = option_env!("MOZAK_MAX_LOOPS")
                .map_or(1_000_000, |env_var| env_var.parse().unwrap());
            debug_assert!(
                last_state.clk != limit,
                "Looped for longer than MOZAK_MAX_LOOPS"
            );
        }
    }
    Ok(ExecutionRecord {
        executed,
        last_state,
    })
}

#[cfg(test)]
#[allow(clippy::cast_sign_loss)]
#[allow(clippy::cast_possible_wrap)]
mod tests {
    use im::HashMap;
    use proptest::prelude::ProptestConfig;
    use proptest::{prop_assume, proptest};

    use super::*;
    use crate::elf::Program;
    use crate::instruction::{Args, Instruction, Op};
    use crate::test_utils::{
        i16_extra, i32_extra, i8_extra, reg, state_before_final, u16_extra, u32_extra, u8_extra,
    };
    use crate::vm::step;

    fn simple_test_code(
        code: &[Instruction],
        mem: &[(u32, u32)],
        regs: &[(u8, u32)],
    ) -> ExecutionRecord {
        crate::test_utils::simple_test_code(code, mem, regs).1
    }

    proptest! {
        #![proptest_config(ProptestConfig { max_global_rejects: 100_000, .. Default::default() })]
        #[test]
        fn add_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in u32_extra(), rs2_value in u32_extra()) {
            prop_assume!(rs1 != rs2);
            let sum = rs1_value.wrapping_add(rs2_value);
            let e = simple_test_code(
                &[Instruction::new(
                    Op::ADD,
                    Args {
                        rd,
                        rs1,
                        rs2,
                        ..Args::default()
                    }
                )],
                &[],
                &[(rs1, rs1_value), (rs2, rs2_value)]
            );
            assert_eq!(state_before_final(&e).get_register_value(rd), sum);
        }

        #[test]
        fn addi_proptest(rd in reg(), rs1 in reg(), rs1_value in u32_extra(), imm in u32_extra()) {
            let e = simple_test_code(
                &[Instruction::new(
                    Op::ADD,
                    Args {
                        rd,
                        rs1,
                        imm,
                        ..Args::default()
                    }
                )],
                &[],
                &[(rs1, rs1_value)]
            );
            assert_eq!(state_before_final(&e).get_register_value(rd), rs1_value.wrapping_add(imm));
        }

        #[test]
        fn sll_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in u32_extra(), rs2_value in u32_extra()) {
            prop_assume!(rs1 != rs2);
            let e = simple_test_code(
                &[Instruction::new(
                    Op::SLL,
                    Args {
                        rd,
                        rs1,
                        rs2,
                        ..Args::default()
                    }
                )],
                &[],
                &[(rs1, rs1_value), (rs2, rs2_value)]
            );
            assert_eq!(
                state_before_final(&e).get_register_value(rd),
                rs1_value << (rs2_value & 0b1_1111)
            );
        }

        #[test]
        fn and_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in u32_extra(), rs2_value in u32_extra()) {
            prop_assume!(rs1 != rs2);
            let e = simple_test_code(
                &[Instruction::new(
                    Op::AND,
                    Args { rd,
                    rs1,
                    rs2,
                        ..Args::default()
                        }
                )],
                &[],
                &[(rs1, rs1_value), (rs2, rs2_value)]
            );
            assert_eq!(
                state_before_final(&e).get_register_value(rd),
                rs1_value & rs2_value
            );
        }

        #[test]
        fn andi_proptest(rd in reg(), rs1 in reg(), rs1_value in u32_extra(), imm in u32_extra()) {
            let e = simple_test_code(
                &[Instruction::new(
                    Op::AND,
                    Args { rd,
                    rs1,
                    imm,
                        ..Args::default()
                        }
                )],
                &[],
                &[(rs1, rs1_value)]
            );
            let expected_value = rs1_value & imm;
            assert_eq!(
                state_before_final(&e).get_register_value(rd),
                expected_value
            );
        }


        #[test]
        fn srl_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in u32_extra(), rs2_value in u32_extra()) {
            prop_assume!(rs1 != rs2);
            let e = simple_test_code(
                &[Instruction::new(
                    Op::SRL,
                    Args { rd,
                    rs1,
                    rs2,
                        ..Args::default()
                        }
                )],
                &[],
                &[(rs1, rs1_value), (rs2, rs2_value)]
            );
            assert_eq!(
                state_before_final(&e).get_register_value(rd),
                rs1_value >> (rs2_value & 0b1_1111)
            );
        }

        #[test]
        fn srli_proptest(rd in reg(), rs1 in reg(), rs1_value in u32_extra(), imm in u32_extra()) {
            let e = simple_test_code(
                &[Instruction::new(
                    Op::SRL,
                    Args { rd,
                    rs1,
                    imm,
                        ..Args::default()
                        }
                )],
                &[],
                &[(rs1, rs1_value)]
            );
            assert_eq!(
                state_before_final(&e).get_register_value(rd),
                rs1_value >> (imm & 0b1_1111)
            );
        }

        #[test]
        fn or_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in u32_extra(), rs2_value in u32_extra()) {
            prop_assume!(rs1 != rs2);
            let e = simple_test_code(
                &[Instruction::new(
                    Op::OR,
                    Args { rd,
                    rs1,
                    rs2,
                        ..Args::default()
                        }
                )],
                &[],
                &[(rs1, rs1_value), (rs2, rs2_value)]
            );
            assert_eq!(
                state_before_final(&e).get_register_value(rd),
                rs1_value | rs2_value
            );
        }

        #[test]
        fn ori_proptest(rd in reg(), rs1 in reg(), rs1_value in u32_extra(), imm in u32_extra()) {
            let e = simple_test_code(
                &[Instruction::new(
                    Op::OR,
                    Args { rd,
                    rs1,
                    imm,
                        ..Args::default()
                        }
                )],
                &[],
                &[(rs1, rs1_value)]
            );
            let expected_value = rs1_value | imm;
            assert_eq!(
                state_before_final(&e).get_register_value(rd),
                expected_value
            );
        }

        #[test]
        fn xor_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in u32_extra(), rs2_value in u32_extra()) {
            prop_assume!(rs1 != rs2);
            let e = simple_test_code(
                &[Instruction::new(
                    Op::XOR,
                    Args { rd,
                    rs1,
                    rs2,
                        ..Args::default()
                        }
                )],
                &[],
                &[(rs1, rs1_value), (rs2, rs2_value)]
            );
            assert_eq!(
                state_before_final(&e).get_register_value(rd),
                rs1_value ^ rs2_value
            );
        }

        #[test]
        fn xori_proptest(rd in reg(), rs1 in reg(), rs1_value in u32_extra(), imm in u32_extra()) {
            let e = simple_test_code(
                &[Instruction::new(
                    Op::XOR,
                    Args { rd,
                    rs1,
                    imm ,
                        ..Args::default()
                        }
                )],
                &[],
                &[(rs1, rs1_value)]
            );
            let expected_value = rs1_value ^ imm;
            assert_eq!(
                state_before_final(&e).get_register_value(rd),
                expected_value
            );
        }

        #[test]
        fn sra_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in u32_extra(), rs2_value in u32_extra()) {
            prop_assume!(rs1 != rs2);
            let e = simple_test_code(
                &[Instruction::new(
                    Op::SRA,
                    Args { rd,
                    rs1,
                    rs2,
                        ..Args::default()
                        }
                )],
                &[],
                &[(rs1, rs1_value), (rs2, rs2_value)]
            );
            assert_eq!(
                state_before_final(&e).get_register_value(rd),
                (rs1_value as i32 >> (rs2_value & 0b1_1111) as i32) as u32
            );
        }

        #[test]
        fn srai_proptest(rd in reg(), rs1 in reg(), rs1_value in u32_extra(), imm in u32_extra()) {
            let e = simple_test_code(
                &[Instruction::new(
                    Op::SRA,
                    Args { rd,
                    rs1,
                    imm,
                        ..Args::default()
                        }
                )],
                &[],
                &[(rs1, rs1_value)]
            );
            let expected_value = (rs1_value as i32 >> (imm & 0b1_1111)) as u32;
            assert_eq!(
                state_before_final(&e).get_register_value(rd),
                expected_value
            );
        }

        #[test]
        fn slt_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in u32_extra(), rs2_value in u32_extra()) {
            prop_assume!(rs1 != rs2);
            let e = simple_test_code(
                &[Instruction::new(
                    Op::SLT,
                    Args { rd,
                    rs1,
                    rs2,
                        ..Args::default()
                        }
                )],
                &[],
                &[(rs1, rs1_value), (rs2, rs2_value)]
            );
            let rs1_value = rs1_value as i32;
            let rs2_value = rs2_value as i32;
            assert_eq!(
                state_before_final(&e).get_register_value(rd),
                u32::from(rs1_value < rs2_value)
            );
        }

        #[test]
        fn sltu_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in u32_extra(), rs2_value in u32_extra()) {
            prop_assume!(rs1 != rs2);
            let e = simple_test_code(
                &[Instruction::new(
                    Op::SLTU,
                    Args { rd,
                    rs1,
                    rs2,
                    ..Args::default()
                }
                )],
                &[],
                &[(rs1, rs1_value), (rs2, rs2_value)]
            );
            assert_eq!(
                state_before_final(&e).get_register_value(rd),
                u32::from(rs1_value < rs2_value)
            );
        }

        #[test]
        fn slti_proptest(rd in reg(), rs1 in reg(), rs1_value in u32_extra(), imm in u32_extra()) {
            let e = simple_test_code(
                &[Instruction::new(
                    Op::SLT,
                    Args { rd,
                    rs1,
                    imm,
                    ..Args::default()
                }
                )],
                &[],
                &[(rs1, rs1_value)]
            );
            assert_eq!(state_before_final(&e).get_register_value(rd), u32::from((rs1_value as i32) < (imm as i32)));
        }

        #[test]
        fn sltiu_proptest(rd in reg(), rs1 in reg(), rs1_value in u32_extra(), imm in u32_extra()) {
            let e = simple_test_code(
                &[Instruction::new(
                    Op::SLTU,
                    Args { rd,
                    rs1,
                    imm,
                    ..Args::default()
                }
                )],
                &[],
                &[(rs1, rs1_value)]
            );
            assert_eq!(state_before_final(&e).get_register_value(rd), u32::from(rs1_value < imm));
        }

        #[test]
        fn slli_proptest(rd in reg(), rs1 in reg(), rs1_value in u32_extra(), imm in u32_extra()) {
            let e = simple_test_code(
                &[Instruction::new(
                    Op::SLL,
                    Args { rd,
                    rs1,
                    imm,
                    ..Args::default()
                }
                )],
                &[],
                &[(rs1, rs1_value)]
            );
            assert_eq!(state_before_final(&e).get_register_value(rd), rs1_value << (imm & 0b1_1111));
        }

        #[test]
        fn lb_proptest(rd in reg(), rs2 in reg(), rs2_value in u32_extra(), offset in u32_extra(), memory_value in i8_extra()) {
            let address = rs2_value.wrapping_add(offset);

            let e = simple_test_code(
                &[Instruction::new(
                    Op::LB,
                    Args { rd,
                    rs2,
                    imm: offset,
                    ..Args::default()
                }

                )],
                &[(address, memory_value as u32)],
                &[(rs2, rs2_value)]
            );

            let expected_value = i32::from(memory_value) as u32;
            assert_eq!(state_before_final(&e).get_register_value(rd), expected_value);
        }

        #[test]
        fn lbu_proptest(rd in reg(), rs2 in reg(), rs2_value in u32_extra(), offset in u32_extra(), memory_value in u8_extra()) {
            let address = rs2_value.wrapping_add(offset);

            let e = simple_test_code(
                &[Instruction::new(
                    Op::LBU,
                    Args { rd,
                    rs2,
                    imm: offset,
                    ..Args::default()
                }

                )],
                &[(address, u32::from(memory_value))],
                &[(rs2, rs2_value)]
            );
            assert_eq!(state_before_final(&e).get_register_value(rd), u32::from(memory_value));
        }

        #[test]
        fn lh_proptest(rd in reg(), rs2 in reg(), rs2_value in u32_extra(), offset in u32_extra(), memory_value in i16_extra()) {
            let address = rs2_value.wrapping_add(offset);

            let e = simple_test_code(
                &[Instruction::new(
                    Op::LH,
                    Args { rd,
                    rs2,
                    imm: offset,
                    ..Args::default()
                }

                )],
                &[(address, u32::from(memory_value as u16))],
                &[(rs2, rs2_value)]
            );
            assert_eq!(state_before_final(&e).get_register_value(rd), i32::from(memory_value) as u32);
        }

        #[test]
        fn lhu_proptest(rd in reg(), rs2 in reg(), rs2_value in u32_extra(), offset in u32_extra(), memory_value in u16_extra()) {
            let address = rs2_value.wrapping_add(offset);

            let e = simple_test_code(
                &[Instruction::new(
                    Op::LHU,
                    Args { rd,
                    rs2,
                    imm: offset,
                    ..Args::default()
                }

                )],
                &[(address, u32::from(memory_value))],
                &[(rs2, rs2_value)]
            );

            assert_eq!(state_before_final(&e).get_register_value(rd), u32::from(memory_value));
        }

        #[test]
        fn lw_proptest(rd in reg(), rs2 in reg(), rs2_value in u32_extra(), offset in u32_extra(), memory_value in u32_extra()) {
            let address = rs2_value.wrapping_add(offset);

            let e = simple_test_code(
                &[Instruction::new(
                    Op::LW,
                    Args { rd,
                    rs2,
                    imm: offset,
                    ..Args::default()
                }
                )],
                &[(address, memory_value)],
                &[(rs2, rs2_value)]
            );
            assert_eq!(state_before_final(&e).get_register_value(rd), memory_value);
        }

        #[test]
        fn sb_proptest(rs1 in reg(), rs1_val in u32_extra(), rs2 in reg(), rs2_val in u32_extra(), offset in u32_extra()) {
            prop_assume!(rs1 != rs2);
            let address = rs2_val.wrapping_add(offset);
            let e = simple_test_code(
                &[Instruction::new(
                    Op::SB,
                    Args {rs1,
                    rs2,
                    imm: offset,
                    ..Args::default()
                }
                )],
                &[(address, 0x0)],
                &[(rs1, rs1_val), (rs2, rs2_val)]
            );

            assert_eq!(u32::from(state_before_final(&e).load_u8(address)), rs1_val & 0xff);
        }

        #[test]
        fn sh_proptest(rs1 in reg(), rs1_val in u32_extra(), rs2 in reg(), rs2_val in u32_extra(), offset in u32_extra()) {
            prop_assume!(rs1 != rs2);
            let address = rs2_val.wrapping_add(offset);
            let e = simple_test_code(
                &[Instruction::new(
                    Op::SH,
                    Args {
                    rs1,
                    rs2,
                    imm: offset,
                    ..Args::default()
                }

                )],
                &[(address, 0x0)],
                &[(rs1, rs1_val), (rs2, rs2_val)]
            );
            // lh will return [0, 1] as LSBs and will set MSBs to 0xFFFF
            let state = state_before_final(&e);
            let memory_value = lh(
                &[
                    state.load_u8(address),
                    state.load_u8(address.wrapping_add(1)),
                    state.load_u8(address.wrapping_add(2)),
                    state.load_u8(address.wrapping_add(3))
                ]
            );
            assert_eq!(memory_value & 0xffff, rs1_val & 0xffff);
        }

        #[test]
        fn sw_proptest(rs1 in reg(), rs1_val in u32_extra(), rs2 in reg(), rs2_val in u32_extra(), offset in u32_extra()) {
            prop_assume!(rs1 != rs2);
            let address = rs2_val.wrapping_add(offset);
            let e = simple_test_code(
                &[Instruction::new(
                    Op::SW,
                Args {
                    rs1,
                    rs2,
                    imm: offset,
                    ..Args::default()
                }
                )],
                &[(address, 0x0)],
                &[(rs1, rs1_val), (rs2, rs2_val)]
            );

            let state = state_before_final(&e);
            let memory_value = lw(
                &[
                    state.load_u8(address),
                    state.load_u8(address.wrapping_add(1)),
                    state.load_u8(address.wrapping_add(2)),
                    state.load_u8(address.wrapping_add(3))
                ]
            );
            assert_eq!(memory_value, rs1_val);
        }

        #[test]
        #[allow(clippy::cast_possible_truncation)]
        fn mul_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in u32_extra(), rs2_value in u32_extra()) {
            prop_assume!(rs1 != rs2);
            let prod = rs1_value.wrapping_mul(rs2_value);
            let e = simple_test_code(
                &[Instruction::new(
                    Op::MUL,
                    Args { rd,
                    rs1,
                    rs2,
                    ..Args::default()
                }
                )],
                &[],
                &[(rs1, rs1_value), (rs2, rs2_value)]
            );
            assert_eq!(state_before_final(&e).get_register_value(rd), prod);
        }

        #[test]
        #[allow(clippy::cast_possible_truncation)]
        fn mulh_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in i32_extra(), rs2_value in i32_extra()) {
            prop_assume!(rs1 != rs2);
            let prod: i64 = i64::from(rs1_value) * i64::from(rs2_value);
            let e = simple_test_code(
                &[Instruction::new(
                    Op::MULH,
                    Args { rd,
                    rs1,
                    rs2,
                    ..Args::default()
                }
                )],
                &[],
                &[(rs1, rs1_value as u32), (rs2, rs2_value as u32)]
            );
            assert_eq!(state_before_final(&e).get_register_value(rd), (prod >> 32) as u32);
        }

        #[test]
        #[allow(clippy::cast_possible_truncation)]
        fn mulhu_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in u32_extra(), rs2_value in u32_extra()) {
            prop_assume!(rs1 != rs2);
            let prod: u64 = u64::from(rs1_value) * u64::from(rs2_value);
            let e = simple_test_code(
                &[Instruction::new(
                    Op::MULHU,
                    Args { rd,
                    rs1,
                    rs2,
                    ..Args::default()
                }
                )],
                &[],
                &[(rs1, rs1_value), (rs2, rs2_value)]
            );
            assert_eq!(state_before_final(&e).get_register_value(rd), (prod >> 32) as u32);
        }

        #[test]
        #[allow(clippy::cast_possible_truncation)]
        fn mulhsu_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in i32_extra(), rs2_value in u32_extra()) {
            prop_assume!(rs1 != rs2);
            let prod: i64 = i64::from(rs1_value) * i64::from(rs2_value);
            let e = simple_test_code(
                &[Instruction::new(
                    Op::MULHSU,
                    Args { rd,
                    rs1,
                    rs2,
                    ..Args::default()
                }
                )],
                &[],
                &[(rs1, rs1_value as u32), (rs2, rs2_value)]
            );
            assert_eq!(state_before_final(&e).get_register_value(rd), (prod >> 32) as u32);
        }

        #[test]
        #[allow(clippy::cast_possible_truncation)]
        fn div_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in i32_extra(), rs2_value in i32_extra()) {
            prop_assume!(rs1 != rs2);
            prop_assume!(rs2_value != 0);
            let e = simple_test_code(
                &[Instruction::new(
                    Op::DIV,
                    Args { rd,
                    rs1,
                    rs2,
                    ..Args::default()
                }
                )],
                &[],
                &[(rs1, rs1_value as u32), (rs2, rs2_value as u32)]
            );
            assert_eq!(state_before_final(&e).get_register_value(rd), div(rs1_value as u32, rs2_value as u32));
        }

        #[test]
        fn divu_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in u32_extra(), rs2_value in u32_extra()) {
            prop_assume!(rs1 != rs2);
            prop_assume!(rs2_value != 0);
            let e = simple_test_code(
                &[Instruction::new(
                    Op::DIVU,
                    Args { rd,
                    rs1,
                    rs2,
                    ..Args::default()
                }
                )],
                &[],
                &[(rs1, rs1_value), (rs2, rs2_value)]
            );
            assert_eq!(state_before_final(&e).get_register_value(rd), divu(rs1_value, rs2_value));
        }

        #[test]
        fn rem_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in i32_extra(), rs2_value in i32_extra()) {
            prop_assume!(rs1 != rs2);
            prop_assume!(rs2_value != 0);
            prop_assume!(rs1_value != i32::min_value() && rs2_value != -1);
            let rem = rs1_value % rs2_value;
            let e = simple_test_code(
                &[Instruction::new(
                    Op::REM,
                    Args { rd,
                    rs1,
                    rs2,
                    ..Args::default()
                }
                )],
                &[],
                &[(rs1, rs1_value as u32), (rs2, rs2_value as u32)]
            );
            assert_eq!(state_before_final(&e).get_register_value(rd), rem as u32);
        }

        #[test]
        fn remu_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in u32_extra(), rs2_value in u32_extra()) {
            prop_assume!(rs1 != rs2);
            prop_assume!(rs2_value != 0);
            let rem = rs1_value % rs2_value;
            let e = simple_test_code(
                &[Instruction::new(
                    Op::REMU,
                    Args { rd,
                    rs1,
                    rs2,
                    ..Args::default()
                }
                )],
                &[],
                &[(rs1, rs1_value), (rs2, rs2_value)]
            );
            assert_eq!(state_before_final(&e).get_register_value(rd), rem);
        }

        #[test]
        fn beq_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in u32_extra(), rs2_value in u32_extra()) {
            prop_assume!(rs1 != rs2);
            prop_assume!(rd != rs1);
            prop_assume!(rd != rs2);
            let e = simple_test_code(
                &[  // rs1 == rs1: take imm-path (8)
                    Instruction::new(
                        Op::BEQ,
                        Args { rd,
                        rs1,
                        rs2: rs1,
                        imm: 8,  // branch target
                }
                    ),
                    Instruction::new(
                        Op::SUB,
                        Args { rd: rs1,
                        rs1,
                        rs2,

                    ..Args::default()
                }
                    ),
                    Instruction::new(
                        Op::ADD,
                        Args { rd,
                        rs1,
                        rs2,
                        ..Args::default()
                    }
                    ),
                ],
                &[],
                &[(rs1, rs1_value), (rs2, rs2_value)]
            );
            assert_eq!(state_before_final(&e).get_register_value(rd), rs1_value.wrapping_add(rs2_value));
        }

        #[test]
        fn bne_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in u32_extra(), rs2_value in u32_extra()) {
            prop_assume!(rs1 != rs2);
            prop_assume!(rs1_value != rs2_value);

            let e = simple_test_code(
                &[  // rs1 != rs2: take imm-path (8)
                    Instruction::new(
                        Op::BNE,
                        Args { rd,
                        rs1,
                        rs2,
                        imm: 8,  // branch target
                    }
                    ),
                    Instruction::new(
                        Op::SUB,
                        Args { rd: rs1,
                        rs1,
                        rs2,
                        ..Args::default()
                    }
                    ),
                    Instruction::new(
                        Op::ADD,
                        Args { rd,
                        rs1,
                        rs2,
                        ..Args::default()
                    }
                    ),
                ],
                &[],
                &[(rs1, rs1_value), (rs2, rs2_value)]
            );
            assert_eq!(state_before_final(&e).get_register_value(rd), rs1_value.wrapping_add(rs2_value));
        }

        #[test]
        fn blt_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in i32_extra(), rs2_value in i32_extra()) {
            prop_assume!(rs1 != rs2);
            prop_assume!(rd != rs1);
            prop_assume!(rd != rs2);
            prop_assume!(rs1_value < rs2_value);

            let e = simple_test_code(
                &[
                    Instruction::new(
                        Op::BLT,
                        Args { rd,
                        rs1,
                        rs2,
                        imm: 8,  // branch target
                    }
                    ),
                    Instruction::new(
                        Op::SUB,
                        Args {rd: rs1,
                        rs1,
                        rs2,
                        ..Args::default()
                    }
                    ),
                    Instruction::new(
                        Op::ADD,
                        Args { rd,
                        rs1,
                        rs2,
                        ..Args::default()
                    }
                    ),
                ],
                &[],
                &[(rs1, rs1_value as u32), (rs2, rs2_value as u32)]
            );
            assert_eq!(state_before_final(&e).get_register_value(rd), rs1_value.wrapping_add(rs2_value) as u32);
        }

        #[test]
        fn bltu_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in u32_extra(), rs2_value in u32_extra()) {
            prop_assume!(rs1 != rs2);
            prop_assume!(rd != rs1);
            prop_assume!(rd != rs2);
            prop_assume!(rs1_value < rs2_value);

            let e = simple_test_code(
                &[
                    Instruction::new(
                        Op::BLTU,
                        Args { rd,
                        rs1,
                        rs2,
                        imm: 8,  // branch target
                    }
                    ),
                    Instruction::new(
                        Op::SUB,
                        Args { rd: rs1,
                        rs1,
                        rs2,
                        ..Args::default()
                        }
                    ),
                    Instruction::new(
                        Op::ADD,
                        Args { rd,
                        rs1,
                        rs2,
                        ..Args::default()
                        }
                    ),
                ],
                &[],
                &[(rs1, rs1_value), (rs2, rs2_value)]
            );
            assert_eq!(state_before_final(&e).get_register_value(rd), rs1_value.wrapping_add(rs2_value));
        }

        #[test]
        fn bge_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in i32_extra(), rs2_value in i32_extra()) {
            prop_assume!(rs1 != rs2);
            prop_assume!(rd != rs1);
            prop_assume!(rd != rs2);
            prop_assume!(rs1_value >= rs2_value);

            let e = simple_test_code(
                &[
                    Instruction::new(
                        Op::BGE,
                        Args { rd,
                        rs1,
                        rs2,
                        imm: 8,  // branch target
                        }
                    ),
                    Instruction::new(
                        Op::SUB,
                        Args { rd: rs1,
                        rs1,
                        rs2,
                        ..Args::default()
                        }
                    ),
                    Instruction::new(
                        Op::ADD,
                        Args { rd,
                        rs1,
                        rs2,
                        ..Args::default()
                        }
                    ),
                ],
                &[],
                &[(rs1, rs1_value as u32), (rs2, rs2_value as u32)]
            );
            assert_eq!(state_before_final(&e).get_register_value(rd), rs1_value.wrapping_add(rs2_value) as u32);
        }

        #[test]
        fn bgeu_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in u32_extra(), rs2_value in u32_extra()) {
            prop_assume!(rs1 != rs2);
            prop_assume!(rd != rs1);
            prop_assume!(rd != rs2);
            prop_assume!(rs1_value >= rs2_value);

            let e = simple_test_code(
                &[
                    Instruction::new(
                        Op::BGEU,
                        Args {
                                    rd,
                        rs1,
                        rs2,
                        imm: 8,  // branch target
                        }
                    ),
                    Instruction::new(
                        Op::SUB,
                                Args {
                        rd: rs1,
                        rs1,
                        rs2,
                        imm: 0,  // branch target
                        }
                    ),
                    Instruction::new(
                        Op::ADD,
                        Args {
                        rd,
                        rs1,
                        rs2,
                        ..Args::default()
                        }
                    ),
                ],
                &[],
                &[(rs1, rs1_value), (rs2, rs2_value)]
            );
            assert_eq!(state_before_final(&e).get_register_value(rd), rs1_value.wrapping_add(rs2_value));
        }

        #[test]
        fn jal_jalr_proptest(imm in 0_u32..3) {
            let imm_value_fixed = 4 * imm + 4; // 4 * (0..3) + 4 = 4, 8, 12, 16
            let inst = Instruction::new( // imm = 0, jump = 4, 1+1 + 1 + 1 + 1 = 5
                        Op::ADD,
                        Args {
                        rd: 2,
                        rs1: 2,
                        rs2: 3,
                    ..Args::default()
            }
                    );
            let e = simple_test_code(
                &[
                    Instruction::new(
                        Op::JALR,
                        Args {
                            imm: imm_value_fixed,
                            ..Args::default()
                        }
                    ),
                    inst,
                    inst,
                    inst,
                    inst,
                ],
                &[],
                &[(2, 1), (3, 1)],
            );
            assert_eq!(state_before_final(&e).get_register_value(2), 5 - imm);
        }
    }

    #[must_use]
    #[allow(clippy::missing_panics_doc)]
    fn simple_test(exit_at: u32, mem: &[(u32, u32)], regs: &[(u8, u32)]) -> ExecutionRecord {
        // TODO(Matthias): stick this line into proper common setup?
        let _ = env_logger::try_init();
        let exit_inst =
        // set sys-call EXIT in x17(or a7)
        &[(exit_at, 0x05d0_0893_u32),
        // add ECALL to halt the program
        (exit_at + 4, 0x0000_0073_u32)];

        let image: HashMap<u32, u32> = mem.iter().chain(exit_inst.iter()).copied().collect();
        let program = Program::from(image);

        let state = regs.iter().fold(State::from(&program), |state, (rs, val)| {
            state.set_register_value(*rs, *val)
        });

        let record = step(&program, state).unwrap();
        assert!(record.last_state.has_halted());
        record
    }

    // NOTE: For writing test cases please follow RISCV
    // calling convention for using registers in instructions.
    // Please check https://en.wikichip.org/wiki/risc-v/registers

    #[test]
    fn ecall() {
        let _ = simple_test_code(&[Instruction::new(Op::ECALL, Args::default())], &[], &[]);
    }

    #[test]
    fn lui() {
        // at 0 address instruction lui
        // LUI x1, -524288
        let ExecutionRecord { last_state, .. } = simple_test(4, &[(0_u32, 0x8000_00b7)], &[]);
        assert_eq!(last_state.get_register_value(1), 0x8000_0000);
        assert_eq!(last_state.get_register_value(1) as i32, -2_147_483_648);
    }

    #[test]
    fn auipc() {
        // at 0 address addi x0, x0, 0
        let ExecutionRecord { last_state, .. } = simple_test(
            8,
            &[
                (0_u32, 0x0000_0013),
                // at 4 address instruction auipc
                // auipc x1, -524288
                (4_u32, 0x8000_0097),
            ],
            &[],
        );
        assert_eq!(last_state.get_register_value(1), 0x8000_0004);
        assert_eq!(last_state.get_register_value(1) as i32, -2_147_483_644);
    }

    #[test]
    fn system_opcode_instructions() {
        let _ = simple_test(
            20,
            &[
                // mret
                (0_u32, 0x3020_0073),
                // csrrs, t5, mcause
                (4_u32, 0x3420_2f73),
                // csrrw, mtvec, t0
                (8_u32, 0x3052_9073),
                // csrrwi, 0x744, 8
                (12_u32, 0x7444_5073),
                // fence, iorw, iorw
                (16_u32, 0x0ff0_000f),
            ],
            &[],
        );
    }
}

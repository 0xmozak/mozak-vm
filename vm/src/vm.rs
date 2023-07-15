use anyhow::Result;

use crate::instruction::{Args, Op};
use crate::state::{Aux, State};

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
        (
            Aux {
                will_halt: true,
                ..Aux::default()
            },
            if self.get_register_value(17) == 93 {
                // Note: we don't advance the program counter for 'halt'.
                // That is we treat 'halt' like an endless loop.
                self.halt() // exit system call
            } else {
                self.bump_pc()
            },
        )
    }

    #[must_use]
    pub fn store(self, inst: &Args, bytes: u32) -> (Aux, Self) {
        let addr = self.get_register_value(inst.rs1).wrapping_add(inst.imm);
        let dst_val: u32 = self.get_register_value(inst.rs2);
        (
            Aux {
                dst_val,
                mem_addr: Some(addr),
                ..Default::default()
            },
            (0..bytes)
                .map(|i| addr.wrapping_add(i))
                .zip(dst_val.to_le_bytes().into_iter())
                .fold(self, |acc, (i, byte)| acc.store_u8(i, byte))
                .bump_pc(),
        )
    }

    #[must_use]
    #[allow(clippy::cast_sign_loss)]
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_possible_wrap)]
    pub fn execute_instruction(self) -> (Aux, Self) {
        let inst = self.current_instruction();
        macro_rules! x_op {
            ($op: expr) => {
                self.register_op(&inst.args, $op)
            };
        }
        macro_rules! rop {
            ($op: expr) => {
                self.register_op(&inst.args, |a, b, _i| $op(a, b))
            };
        }
        let (aux, state) = match inst.op {
            Op::ADD => x_op!(|a, b, i| a.wrapping_add(b.wrapping_add(i))),
            // Only use lower 5 bits of rs2 or imm
            Op::SLL => x_op!(|a, b, i| a << ((b.wrapping_add(i)) & 0x1F)),
            // Only use lower 5 bits of rs2 or imm
            Op::SRL => x_op!(|a, b, i| a >> ((b.wrapping_add(i)) & 0x1F)),
            // Only use lower 5 bits of rs2 or imm
            Op::SRA => x_op!(|a, b, i| (a as i32 >> (b.wrapping_add(i) & 0x1F) as i32) as u32),
            Op::SLT => x_op!(|a, b, i| u32::from((a as i32) < (b as i32).wrapping_add(i as i32))),
            Op::SLTU => x_op!(|a, b, i| u32::from(a < b.wrapping_add(i))),
            Op::AND => x_op!(|a, b, i| core::ops::BitAnd::bitand(a, b.wrapping_add(i))),
            Op::OR => x_op!(|a, b, i| core::ops::BitOr::bitor(a, b.wrapping_add(i))),
            Op::XOR => x_op!(|a, b, i| core::ops::BitXor::bitxor(a, b.wrapping_add(i))),
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
pub fn step(mut last_state: State) -> Result<ExecutionRecord> {
    let mut executed = vec![];
    while !last_state.has_halted() {
        let (aux, new_state) = last_state.clone().execute_instruction();
        executed.push(Row {
            state: last_state,
            aux,
        });
        last_state = new_state;

        if cfg!(debug_assertions) {
            let limit: u64 = std::option_env!("MOZAK_MAX_LOOPS")
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
    use proptest::prelude::ProptestConfig;
    use proptest::{prop_assume, proptest};

    use super::{div, divu, lh, lw, ExecutionRecord};
    use crate::instruction::{Instruction, Op};
    use crate::test_utils::{
        i32_extra, i8_extra, last_but_coda, reg, simple_test, simple_test_code,
        u32_extra,
    };

    proptest! {
        #![proptest_config(ProptestConfig { max_global_rejects: 100_000, .. Default::default() })]
        #[test]
        fn add_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in u32_extra(), rs2_value in u32_extra()) {
            prop_assume!(rs1 != rs2);
            let sum = rs1_value.wrapping_add(rs2_value);
            let e = simple_test_code(
                &[Instruction::new(
                    Op::ADD,
                    rd,
                    rs1,
                    rs2,
                    0,
                )],
                &[],
                &[(rs1, rs1_value), (rs2, rs2_value)]
            );
            assert_eq!(last_but_coda(&e).get_register_value(rd), sum);
        }

        #[test]
        fn addi_proptest(rd in reg(), rs1 in reg(), rs1_value in u32_extra(), imm in u32_extra()) {
            let e = simple_test_code(
                &[Instruction::new(
                    Op::ADD,
                    rd,
                    rs1,
                    0,
                    imm,
                )],
                &[],
                &[(rs1, rs1_value)]
            );
            assert_eq!(last_but_coda(&e).get_register_value(rd), rs1_value.wrapping_add(imm));
        }

        #[test]
        fn sll_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in u32_extra(), rs2_value in u32_extra()) {
            prop_assume!(rs1 != rs2);
            let e = simple_test_code(
                &[Instruction::new(
                    Op::SLL,
                    rd,
                    rs1,
                    rs2,
                    0,
                )],
                &[],
                &[(rs1, rs1_value), (rs2, rs2_value)]
            );
            assert_eq!(
                last_but_coda(&e).get_register_value(rd),
                rs1_value << (rs2_value & 0x1F)
            );
        }

        #[test]
        fn and_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in u32_extra(), rs2_value in u32_extra()) {
            prop_assume!(rs1 != rs2);
            let e = simple_test_code(
                &[Instruction::new(
                    Op::AND,
                    rd,
                    rs1,
                    rs2,
                    0,
                )],
                &[],
                &[(rs1, rs1_value), (rs2, rs2_value)]
            );
            assert_eq!(
                last_but_coda(&e).get_register_value(rd),
                rs1_value & rs2_value
            );
        }

        #[test]
        fn andi_proptest(rd in reg(), rs1 in reg(), rs1_value in u32_extra(), imm in u32_extra()) {
            let e = simple_test_code(
                &[Instruction::new(
                    Op::AND,
                    rd,
                    rs1,
                    0,
                    imm,
                )],
                &[],
                &[(rs1, rs1_value)]
            );
            let expected_value = rs1_value & imm;
            assert_eq!(
                last_but_coda(&e).get_register_value(rd),
                expected_value
            );
        }


        #[test]
        fn srl_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in u32_extra(), rs2_value in u32_extra()) {
            prop_assume!(rs1 != rs2);
            let e = simple_test_code(
                &[Instruction::new(
                    Op::SRL,
                    rd,
                    rs1,
                    rs2,
                    0,
                )],
                &[],
                &[(rs1, rs1_value), (rs2, rs2_value)]
            );
            assert_eq!(
                last_but_coda(&e).get_register_value(rd),
                rs1_value >> (rs2_value & 0x1F)
            );
        }

        #[test]
        fn srli_proptest(rd in reg(), rs1 in reg(), rs1_value in u32_extra(), imm in u32_extra()) {
            let e = simple_test_code(
                &[Instruction::new(
                    Op::SRL,
                    rd,
                    rs1,
                    0,
                    imm,
                )],
                &[],
                &[(rs1, rs1_value)]
            );
            assert_eq!(
                last_but_coda(&e).get_register_value(rd),
                rs1_value >> (imm & 0x1f)
            );
        }

        #[test]
        fn or_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in u32_extra(), rs2_value in u32_extra()) {
            prop_assume!(rs1 != rs2);
            let e = simple_test_code(
                &[Instruction::new(
                    Op::OR,
                    rd,
                    rs1,
                    rs2,
                    0,
                )],
                &[],
                &[(rs1, rs1_value), (rs2, rs2_value)]
            );
            assert_eq!(
                last_but_coda(&e).get_register_value(rd),
                rs1_value | rs2_value
            );
        }

        #[test]
        fn ori_proptest(rd in reg(), rs1 in reg(), rs1_value in u32_extra(), imm in u32_extra()) {
            let e = simple_test_code(
                &[Instruction::new(
                    Op::OR,
                    rd,
                    rs1,
                    0,
                    imm,
                )],
                &[],
                &[(rs1, rs1_value)]
            );
            let expected_value = rs1_value | imm;
            assert_eq!(
                last_but_coda(&e).get_register_value(rd),
                expected_value
            );
        }

        #[test]
        fn xor_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in u32_extra(), rs2_value in u32_extra()) {
            prop_assume!(rs1 != rs2);
            let e = simple_test_code(
                &[Instruction::new(
                    Op::XOR,
                    rd,
                    rs1,
                    rs2,
                    0,
                )],
                &[],
                &[(rs1, rs1_value), (rs2, rs2_value)]
            );
            assert_eq!(
                last_but_coda(&e).get_register_value(rd),
                rs1_value ^ rs2_value
            );
        }

        #[test]
        fn xori_proptest(rd in reg(), rs1 in reg(), rs1_value in u32_extra(), imm in u32_extra()) {
            let e = simple_test_code(
                &[Instruction::new(
                    Op::XOR,
                    rd,
                    rs1,
                    0,
                    imm ,
                )],
                &[],
                &[(rs1, rs1_value)]
            );
            let expected_value = rs1_value ^ imm;
            assert_eq!(
                last_but_coda(&e).get_register_value(rd),
                expected_value
            );
        }

        #[test]
        fn sra_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in u32_extra(), rs2_value in u32_extra()) {
            prop_assume!(rs1 != rs2);
            let e = simple_test_code(
                &[Instruction::new(
                    Op::SRA,
                    rd,
                    rs1,
                    rs2,
                    0,
                )],
                &[],
                &[(rs1, rs1_value), (rs2, rs2_value)]
            );
            assert_eq!(
                last_but_coda(&e).get_register_value(rd),
                (rs1_value as i32 >> (rs2_value & 0x1F) as i32) as u32
            );
        }

        #[test]
        fn srai_proptest(rd in reg(), rs1 in reg(), rs1_value in u32_extra(), imm in u32_extra()) {
            let e = simple_test_code(
                &[Instruction::new(
                    Op::SRA,
                    rd,
                    rs1,
                    0,
                    imm,
                )],
                &[],
                &[(rs1, rs1_value)]
            );
            let expected_value = (rs1_value as i32 >> (imm & 0x1f)) as u32;
            assert_eq!(
                last_but_coda(&e).get_register_value(rd),
                expected_value
            );
        }

        #[test]
        fn slt_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in u32_extra(), rs2_value in u32_extra()) {
            prop_assume!(rs1 != rs2);
            let e = simple_test_code(
                &[Instruction::new(
                    Op::SLT,
                    rd,
                    rs1,
                    rs2,
                    0,
                )],
                &[],
                &[(rs1, rs1_value), (rs2, rs2_value)]
            );
            let rs1_value = rs1_value as i32;
            let rs2_value = rs2_value as i32;
            assert_eq!(
                last_but_coda(&e).get_register_value(rd),
                u32::from(rs1_value < rs2_value)
            );
        }

        #[test]
        fn sltu_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in u32_extra(), rs2_value in u32_extra()) {
            prop_assume!(rs1 != rs2);
            let e = simple_test_code(
                &[Instruction::new(
                    Op::SLTU,
                    rd,
                    rs1,
                    rs2,
                    0,
                )],
                &[],
                &[(rs1, rs1_value), (rs2, rs2_value)]
            );
            assert_eq!(
                last_but_coda(&e).get_register_value(rd),
                u32::from(rs1_value < rs2_value)
            );
        }

        #[test]
        fn slti_proptest(rd in reg(), rs1 in reg(), rs1_value in u32_extra(), imm in u32_extra()) {
            let e = simple_test_code(
                &[Instruction::new(
                    Op::SLT,
                    rd,
                    rs1,
                    0,
                    imm,
                )],
                &[],
                &[(rs1, rs1_value)]
            );
            assert_eq!(last_but_coda(&e).get_register_value(rd), u32::from((rs1_value as i32) < (imm as i32)));
        }

        #[test]
        fn sltiu_proptest(rd in reg(), rs1 in reg(), rs1_value in u32_extra(), imm in u32_extra()) {
            let e = simple_test_code(
                &[Instruction::new(
                    Op::SLTU,
                    rd,
                    rs1,
                    0,
                    imm,
                )],
                &[],
                &[(rs1, rs1_value)]
            );
            assert_eq!(last_but_coda(&e).get_register_value(rd), u32::from(rs1_value < imm));
        }

        #[test]
        fn slli_proptest(rd in reg(), rs1 in reg(), rs1_value in u32_extra(), imm in u32_extra()) {
            let e = simple_test_code(
                &[Instruction::new(
                    Op::SLL,
                    rd,
                    rs1,
                    0,
                    imm,
                )],
                &[],
                &[(rs1, rs1_value)]
            );
            assert_eq!(last_but_coda(&e).get_register_value(rd), rs1_value << (imm & 0x1F));
        }

        #[test]
        fn lb_proptest(rd in reg(), rs1 in reg(), rs1_value in u32_extra(), offset in u32_extra(), memory_value in i8_extra()) {
            let address = rs1_value.wrapping_add(offset);

            let e = simple_test_code(
                &[Instruction::new(
                    Op::LB,
                    rd,
                    rs1,
                    0,
                    offset,
                )],
                &[(address, memory_value as u32)],
                &[(rs1, rs1_value)]
            );

            let expected_value = i32::from(memory_value) as u32;
            assert_eq!(last_but_coda(&e).get_register_value(rd), expected_value);
        }

        #[test]
        fn lbu_proptest(rd in reg(), rs1 in reg(), rs1_value in u32_extra(), offset in u32_extra(), memory_value in i8_extra()) {
            let address = rs1_value.wrapping_add(offset);

            let e = simple_test_code(
                &[Instruction::new(
                    Op::LBU,
                    rd,
                    rs1,
                    0,
                    offset as u32,
                )],
                &[(address, memory_value as u32)],
                &[(rs1, rs1_value)]
            );

            let expected_value = (memory_value as u32) & 0x0000_00FF;
            assert_eq!(last_but_coda(&e).get_register_value(rd), expected_value);
        }

        #[test]
        fn lh_proptest(rd in reg(), rs1 in reg(), rs1_value in u32_extra(), offset in u32_extra(), memory_value in i8_extra()) {
            let address = rs1_value.wrapping_add(offset);

            let e = simple_test_code(
                &[Instruction::new(
                    Op::LH,
                    rd,
                    rs1,
                    0,
                    offset as u32,
                )],
                &[(address, memory_value as u32)],
                &[(rs1, rs1_value)]
            );

            let expected_value = i32::from(memory_value) as u32;
            assert_eq!(last_but_coda(&e).get_register_value(rd), expected_value);
        }

        #[test]
        fn lhu_proptest(rd in reg(), rs1 in reg(), rs1_value in u32_extra(), offset in u32_extra(), memory_value in i8_extra()) {
            let address = rs1_value.wrapping_add(offset);

            let e = simple_test_code(
                &[Instruction::new(
                    Op::LHU,
                    rd,
                    rs1,
                    0,
                    offset as u32,
                )],
                &[(address, memory_value as u32)],
                &[(rs1, rs1_value)]
            );

            let expected_value = (memory_value as u32) & 0x0000_FFFF;
            assert_eq!(last_but_coda(&e).get_register_value(rd), expected_value);
        }

        #[test]
        fn lw_proptest(rd in reg(), rs1 in reg(), rs1_value in u32_extra(), offset in u32_extra(), memory_value in i8_extra()) {
            let address = rs1_value.wrapping_add(offset);

            let e = simple_test_code(
                &[Instruction::new(
                    Op::LW,
                    rd,
                    rs1,
                    0,
                    offset,
                )],
                &[(address, memory_value as u32)],
                &[(rs1, rs1_value)]
            );

            let expected_value = memory_value as u32;
            assert_eq!(last_but_coda(&e).get_register_value(rd), expected_value);
        }

        #[test]
        fn sb_proptest(rs1 in reg(), rs1_val in u32_extra(), rs2 in reg(), rs2_val in u32_extra(), offset in u32_extra()) {
            prop_assume!(rs1 != rs2);
            let address = rs1_val.wrapping_add(offset);
            let e = simple_test_code(
                &[Instruction::new(
                    Op::SB,
                    0,
                    rs1,
                    rs2,
                    offset
                )],
                &[(address, 0x0)],
                &[(rs1, rs1_val), (rs2, rs2_val)]
            );

            assert_eq!(u32::from(last_but_coda(&e).load_u8(address)), rs2_val & 0xff);
        }

        #[test]
        fn sh_proptest(rs1 in reg(), rs1_val in u32_extra(), rs2 in reg(), rs2_val in u32_extra(), offset in u32_extra()) {
            prop_assume!(rs1 != rs2);
            let address = rs1_val.wrapping_add(offset);
            let e = simple_test_code(
                &[Instruction::new(
                    Op::SH,
                    0,
                    rs1,
                    rs2,
                    offset
                )],
                &[(address, 0x0)],
                &[(rs1, rs1_val), (rs2, rs2_val)]
            );
            // lh will return [0, 1] as LSBs and will set MSBs to 0xFFFF
            let state = last_but_coda(&e);
            let memory_value = lh(
                &[
                    state.load_u8(address),
                    state.load_u8(address.wrapping_add(1)),
                    state.load_u8(address.wrapping_add(2)),
                    state.load_u8(address.wrapping_add(3))
                ]
            );
            assert_eq!(memory_value & 0xffff, rs2_val & 0xffff);
        }

        #[test]
        fn sw_proptest(rs1 in reg(), rs1_val in u32_extra(), rs2 in reg(), rs2_val in u32_extra(), offset in u32_extra()) {
            prop_assume!(rs1 != rs2);
            let address = rs1_val.wrapping_add(offset);
            let e = simple_test_code(
                &[Instruction::new(
                    Op::SW,
                    0,
                    rs1,
                    rs2,
                    offset
                )],
                &[(address, 0x0)],
                &[(rs1, rs1_val), (rs2, rs2_val)]
            );

            let state = last_but_coda(&e);
            let memory_value = lw(
                &[
                    state.load_u8(address),
                    state.load_u8(address.wrapping_add(1)),
                    state.load_u8(address.wrapping_add(2)),
                    state.load_u8(address.wrapping_add(3))
                ]
            );
            assert_eq!(memory_value, rs2_val);
        }

        #[test]
        #[allow(clippy::cast_possible_truncation)]
        fn mul_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in u32_extra(), rs2_value in u32_extra()) {
            prop_assume!(rs1 != rs2);
            let prod = rs1_value.wrapping_mul(rs2_value);
            let e = simple_test_code(
                &[Instruction::new(
                    Op::MUL,
                    rd,
                    rs1,
                    rs2,
                    0,
                )],
                &[],
                &[(rs1, rs1_value), (rs2, rs2_value)]
            );
            assert_eq!(last_but_coda(&e).get_register_value(rd), prod);
        }

        #[test]
        #[allow(clippy::cast_possible_truncation)]
        fn mulh_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in u32_extra(), rs2_value in u32_extra()) {
            prop_assume!(rs1 != rs2);
            let prod: i64 = i64::from(rs1_value as i32) * i64::from(rs2_value as i32);
            let e = simple_test_code(
                &[Instruction::new(
                    Op::MULH,
                    rd,
                    rs1,
                    rs2,
                    0,
                )],
                &[],
                &[(rs1, rs1_value), (rs2, rs2_value)]
            );
            assert_eq!(last_but_coda(&e).get_register_value(rd), (prod >> 32) as u32);
        }

        #[test]
        #[allow(clippy::cast_possible_truncation)]
        fn mulhu_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in u32_extra(), rs2_value in u32_extra()) {
            prop_assume!(rs1 != rs2);
            let prod: u64 = u64::from(rs1_value) * u64::from(rs2_value);
            let e = simple_test_code(
                &[Instruction::new(
                    Op::MULHU,
                    rd,
                    rs1,
                    rs2,
                    0,
                )],
                &[],
                &[(rs1, rs1_value), (rs2, rs2_value)]
            );
            assert_eq!(last_but_coda(&e).get_register_value(rd), (prod >> 32) as u32);
        }

        #[test]
        #[allow(clippy::cast_possible_truncation)]
        fn mulhsu_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in i32_extra(), rs2_value in u32_extra()) {
            prop_assume!(rs1 != rs2);
            let prod: i64 = i64::from(rs1_value) * i64::from(rs2_value);
            let e = simple_test_code(
                &[Instruction::new(
                    Op::MULHSU,
                    rd,
                    rs1,
                    rs2,
                    0,
                )],
                &[],
                &[(rs1, rs1_value as u32), (rs2, rs2_value)]
            );
            assert_eq!(last_but_coda(&e).get_register_value(rd), (prod >> 32) as u32);
        }

        #[test]
        #[allow(clippy::cast_possible_truncation)]
        fn div_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in i32_extra(), rs2_value in i32_extra()) {
            prop_assume!(rs1 != rs2);
            prop_assume!(rs2_value != 0);
            let e = simple_test_code(
                &[Instruction::new(
                    Op::DIV,
                    rd,
                    rs1,
                    rs2,
                    0,
                )],
                &[],
                &[(rs1, rs1_value as u32), (rs2, rs2_value as u32)]
            );
            assert_eq!(last_but_coda(&e).get_register_value(rd), div(rs1_value as u32, rs2_value as u32));
        }

        #[test]
        fn divu_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in u32_extra(), rs2_value in u32_extra()) {
            prop_assume!(rs1 != rs2);
            prop_assume!(rs2_value != 0);
            let e = simple_test_code(
                &[Instruction::new(
                    Op::DIVU,
                    rd,
                    rs1,
                    rs2,
                    0,
                )],
                &[],
                &[(rs1, rs1_value), (rs2, rs2_value)]
            );
            assert_eq!(last_but_coda(&e).get_register_value(rd), divu(rs1_value, rs2_value));
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
                    rd,
                    rs1,
                    rs2,
                    0,
                )],
                &[],
                &[(rs1, rs1_value as u32), (rs2, rs2_value as u32)]
            );
            assert_eq!(last_but_coda(&e).get_register_value(rd), rem as u32);
        }

        #[test]
        fn remu_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in u32_extra(), rs2_value in u32_extra()) {
            prop_assume!(rs1 != rs2);
            prop_assume!(rs2_value != 0);
            let rem = rs1_value % rs2_value;
            let e = simple_test_code(
                &[Instruction::new(
                    Op::REMU,
                    rd,
                    rs1,
                    rs2,
                    0,
                )],
                &[],
                &[(rs1, rs1_value), (rs2, rs2_value)]
            );
            assert_eq!(last_but_coda(&e).get_register_value(rd), rem);
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
                        rd,
                        rs1,
                        rs1,
                        8,
                    ),
                    Instruction::new(
                        Op::SUB,
                        rs1,
                        rs1,
                        rs2,
                        0,
                    ),
                    Instruction::new(
                        Op::ADD,
                        rd,
                        rs1,
                        rs2,
                        0,
                    ),
                ],
                &[],
                &[(rs1, rs1_value), (rs2, rs2_value)]
            );
            assert_eq!(last_but_coda(&e).get_register_value(rd), rs1_value.wrapping_add(rs2_value));
        }

        #[test]
        fn bne_proptest(rd in reg(), rs1 in reg(), rs2 in reg(), rs1_value in u32_extra(), rs2_value in u32_extra()) {
            prop_assume!(rs1 != rs2);
            prop_assume!(rs1_value != rs2_value);

            let e = simple_test_code(
                &[  // rs1 != rs2: take imm-path (8)
                    Instruction::new(
                        Op::BNE,
                        rd,
                        rs1,
                        rs2,
                        8,
                    ),
                    Instruction::new(
                        Op::SUB,
                        rs1,
                        rs1,
                        rs2,
                        0,
                    ),
                    Instruction::new(
                        Op::ADD,
                        rd,
                        rs1,
                        rs2,
                        0,
                    ),
                ],
                &[],
                &[(rs1, rs1_value), (rs2, rs2_value)]
            );
            assert_eq!(last_but_coda(&e).get_register_value(rd), rs1_value.wrapping_add(rs2_value));
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
                        rd,
                        rs1,
                        rs2,
                        8,
                    ),
                    Instruction::new(
                        Op::SUB,
                        rs1,
                        rs1,
                        rs2,
                        0,
                    ),
                    Instruction::new(
                        Op::ADD,
                        rd,
                        rs1,
                        rs2,
                        0,
                    ),
                ],
                &[],
                &[(rs1, rs1_value as u32), (rs2, rs2_value as u32)]
            );
            assert_eq!(last_but_coda(&e).get_register_value(rd), rs1_value.wrapping_add(rs2_value) as u32);
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
                        rd,
                        rs1,
                        rs2,
                        8,
                    ),
                    Instruction::new(
                        Op::SUB,
                        rs1,
                        rs1,
                        rs2,
                        0,
                    ),
                    Instruction::new(
                        Op::ADD,
                        rd,
                        rs1,
                        rs2,
                        0,
                    ),
                ],
                &[],
                &[(rs1, rs1_value), (rs2, rs2_value)]
            );
            assert_eq!(last_but_coda(&e).get_register_value(rd), rs1_value.wrapping_add(rs2_value));
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
                        rd,
                        rs1,
                        rs2,
                        8,
                    ),
                    Instruction::new(
                        Op::SUB,
                        rs1,
                        rs1,
                        rs2,
                        0,
                    ),
                    Instruction::new(
                        Op::ADD,
                        rd,
                        rs1,
                        rs2,
                        0,
                    ),
                ],
                &[],
                &[(rs1, rs1_value as u32), (rs2, rs2_value as u32)]
            );
            assert_eq!(last_but_coda(&e).get_register_value(rd), rs1_value.wrapping_add(rs2_value) as u32);
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
                        rd,
                        rs1,
                        rs2,
                        8,
                    ),
                    Instruction::new(
                        Op::SUB,
                        rs1,
                        rs1,
                        rs2,
                        0,
                    ),
                    Instruction::new(
                        Op::ADD,
                        rd,
                        rs1,
                        rs2,
                        0,
                    ),
                ],
                &[],
                &[(rs1, rs1_value), (rs2, rs2_value)]
            );
            assert_eq!(last_but_coda(&e).get_register_value(rd), rs1_value.wrapping_add(rs2_value));
        }

        #[test]
        fn jal_jalr_proptest(imm in 0_u32..3) {
            let imm_value_fixed = 4 * imm + 4; // 4 * (0..3) + 4 = 4, 8, 12, 16
            let e = simple_test_code(
                &[
                    Instruction::new(
                        Op::JALR,
                        0,
                        0,
                        0,
                        imm_value_fixed,
                    ),
                    Instruction::new( // imm = 0, jump = 4, 1+1 + 1 + 1 + 1 = 5
                        Op::ADD,
                        2,
                        2,
                        2,
                        0,
                    ),
                    Instruction::new( // imm = 1, jump = 8, 1+1 + 1 + 1 = 4
                        Op::ADD,
                        2,
                        2,
                        3,
                        0,
                    ),
                    Instruction::new( // imm = 2, jump = 12, 1+1 + 1 = 3
                        Op::ADD,
                        2,
                        2,
                        3,
                        0,
                    ),
                    Instruction::new( // imm = 3, jump = 16, 1+1 = 2
                        Op::ADD,
                        2,
                        2,
                        3,
                        0,
                    ),
                ],
                &[],
                &[(2, 1), (3, 1)],
            );
            assert_eq!(last_but_coda(&e).get_register_value(2), 5 - imm);
        }
    }

    // NOTE: For writing test cases please follow RISCV
    // calling convention for using registers in instructions.
    // Please check https://en.wikichip.org/wiki/risc-v/registers

    #[test]
    fn ecall() { let _ = simple_test_code(&[Instruction::new(Op::ECALL, 0, 0, 0, 0)], &[], &[]); }

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

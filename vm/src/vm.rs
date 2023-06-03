use alloc::collections::BTreeMap;

use anyhow::Result;

use crate::{
    elf::{Program, Code},
    instruction::{ITypeInst, Instruction, JTypeInst, STypeInst, UTypeInst},
    state::State,
};

#[must_use]
pub fn mulh(a: u32, b: u32) -> u32 {
    ((i64::from(a as i32) * i64::from(b as i32)) >> 32) as u32
}

#[must_use]
pub fn mulhu(a: u32, b: u32) -> u32 {
    ((u64::from(a) * u64::from(b)) >> 32) as u32
}

#[must_use]
pub fn mulhsu(a: u32, b: u32) -> u32 {
    ((i64::from(a as i32) * i64::from(b)) >> 32) as u32
}

#[must_use]
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
pub fn lb(mem: &[u8; 4]) -> u32 {
    i32::from(mem[0] as i8) as u32
}

#[must_use]
pub fn lbu(mem: &[u8; 4]) -> u32 {
    mem[0].into()
}

#[must_use]
pub fn lh(mem: &[u8; 4]) -> u32 {
    i32::from(i16::from_le_bytes([mem[0], mem[1]])) as u32
}

#[must_use]
pub fn lhu(mem: &[u8; 4]) -> u32 {
    u16::from_le_bytes([mem[0], mem[1]]).into()
}

#[must_use]
pub fn lw(mem: &[u8; 4]) -> u32 {
    u32::from_le_bytes(*mem)
}

impl State {
    #[must_use]
    pub fn lui(self, inst: &UTypeInst) -> Self {
        self.set_register_value(inst.rd.into(), inst.imm as u32)
            .bump_pc()
    }

    #[must_use]
    pub fn jal(self, inst: &JTypeInst) -> Self {
        let pc = self.get_pc();
        self.bump_pc_n(inst.imm as u32)
            .set_register_value(inst.rd.into(), pc.wrapping_add(4))
    }

    #[must_use]
    pub fn jalr(self, inst: &ITypeInst) -> Self {
        let pc = self.get_pc();
        let new_pc = (self
            .get_register_value(inst.rs1.into())
            .wrapping_add(inst.imm as u32))
            & !1;
        self.set_pc(new_pc)
            .set_register_value(inst.rd.into(), pc.wrapping_add(4))
    }

    #[must_use]
    pub fn ecall(self) -> Self {
        if self.get_register_value(17_usize) == 93 {
            self.halt() // exit system call
        } else {
            self
        }
        .bump_pc()
    }

    #[must_use]
    pub fn auipc(self, inst: &UTypeInst) -> Self {
        let res = self.get_pc().wrapping_add(inst.imm as u32);
        self.set_register_value(inst.rd.into(), res).bump_pc()
    }

    #[must_use]
    pub fn store(self, inst: &STypeInst, bytes: usize) -> Self {
        let addr = self
            .get_register_value(inst.rs1.into())
            .wrapping_add(inst.imm as u32);
        let value: u32 = self.get_register_value(inst.rs2.into());
        (value.to_le_bytes()[0..bytes])
            .iter()
            .enumerate()
            .fold(self, |acc, (i, byte)| {
                acc.store_u8(addr.wrapping_add(i as u32), *byte)
            })
            .bump_pc()
    }

    #[must_use]
    pub fn execute_instruction<F>(self, get_instruction: F) -> Self where
        F: FnOnce(u32) -> &'static Instruction
        {
        let inst = get_instruction(self.get_pc());
        match inst {
            Instruction::ADD(inst) => inst.register_op(self, u32::wrapping_add),
            Instruction::ADDI(inst) => inst.register_op(self, u32::wrapping_add),
            // Only use lower 5 bits of rs2
            Instruction::SLL(inst) => inst.register_op(self, |a, b| a << (b & 0x1F)),
            // Only use lower 5 bits of rs2
            Instruction::SRL(inst) => inst.register_op(self, |a, b| a >> (b & 0x1F)),
            // Only use lower 5 bits of rs2
            Instruction::SRA(inst) => {
                inst.register_op(self, |a, b| (a as i32 >> (b & 0x1F) as i32) as u32)
            }
            Instruction::SLT(inst) => {
                inst.register_op(self, |a, b| u32::from((a as i32) < (b as i32)))
            }
            Instruction::SLTU(inst) => inst.register_op(self, |a, b| u32::from(a < b)),
            Instruction::SRAI(inst) => inst.register_op(self, |a, b| ((a as i32) >> b) as u32),
            Instruction::SRLI(inst) => inst.register_op(self, core::ops::Shr::shr),
            Instruction::SLLI(inst) => inst.register_op(self, |a, b| a << b),
            Instruction::SLTI(inst) => {
                inst.register_op(self, |a, b| u32::from((a as i32) < b as i32))
            }
            Instruction::SLTIU(inst) => inst.register_op(self, |a, b| u32::from(a < b)),
            Instruction::AND(inst) => inst.register_op(self, core::ops::BitAnd::bitand),
            Instruction::ANDI(inst) => inst.register_op(self, core::ops::BitAnd::bitand),
            Instruction::OR(inst) => inst.register_op(self, core::ops::BitOr::bitor),
            Instruction::ORI(inst) => inst.register_op(self, core::ops::BitOr::bitor),
            Instruction::XOR(inst) => inst.register_op(self, core::ops::BitXor::bitxor),
            Instruction::XORI(inst) => inst.register_op(self, core::ops::BitXor::bitxor),
            Instruction::SUB(inst) => inst.register_op(self, u32::wrapping_sub),

            Instruction::LB(inst) => inst.memory_load(self, lb),
            Instruction::LBU(inst) => inst.memory_load(self, lbu),
            Instruction::LH(inst) => inst.memory_load(self, lh),
            Instruction::LHU(inst) => inst.memory_load(self, lhu),
            Instruction::LW(inst) => inst.memory_load(self, lw),
            Instruction::ECALL => self.ecall(),
            Instruction::JAL(inst) => self.jal(&inst),
            Instruction::JALR(inst) => self.jalr(&inst),
            Instruction::BEQ(inst) => inst.register_op(self, |a, b| a == b),
            Instruction::BNE(inst) => inst.register_op(self, |a, b| a != b),
            Instruction::BLT(inst) => inst.register_op(self, |a, b| (a as i32) < (b as i32)),
            Instruction::BLTU(inst) => inst.register_op(self, |a, b| a < b),
            Instruction::BGE(inst) => inst.register_op(self, |a, b| (a as i32) >= (b as i32)),
            Instruction::BGEU(inst) => inst.register_op(self, |a, b| a >= b),
            Instruction::SW(inst) => self.store(&inst, 4),
            Instruction::SH(inst) => self.store(&inst, 2),
            Instruction::SB(inst) => self.store(&inst, 1),
            Instruction::MUL(inst) => inst.register_op(self, u32::wrapping_mul),
            Instruction::MULH(inst) => inst.register_op(self, mulh),
            Instruction::MULHU(inst) => inst.register_op(self, mulhu),
            Instruction::MULHSU(inst) => inst.register_op(self, mulhsu),
            Instruction::LUI(inst) => self.lui(&inst),
            Instruction::AUIPC(inst) => self.auipc(&inst),
            Instruction::DIV(inst) => inst.register_op(self, div),
            Instruction::DIVU(inst) => inst.register_op(self, divu),
            Instruction::REM(inst) => inst.register_op(self, rem),
            Instruction::REMU(inst) => inst.register_op(self, remu),
            // It's not important that these instructions are implemented for the sake of
            // our purpose at this moment, but these instructions are found in the test
            // data that we use - so we simply advance the register.
            Instruction::FENCE(_)
            | Instruction::CSRRS(_)
            | Instruction::CSRRW(_)
            | Instruction::CSRRWI(_)
            | Instruction::EBREAK
            | Instruction::MRET => self.bump_pc(),
            Instruction::UNKNOWN => unimplemented!("Unknown instruction"),
        }
        .bump_clock()
    }
}

/// Later on, this can hold traces.
#[derive(Debug, Clone, Default)]
pub struct Row {
    pub state: State,
}

pub struct Vm {
    code: Code,
}

impl Vm {

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
pub fn step(&self, mut state: State) -> Result<(Vec<Row>, State)> {
    let mut rows = vec![Row {
        state: state.clone(),
    }];
    while !state.has_halted() {
        state = state.execute_instruction(|pc| &self.code.get_instruction(pc));
        rows.push(Row {
            state: state.clone(),
        });

        if cfg!(debug_assertions) {
            let limit: u32 = std::option_env!("MOZAK_MAX_LOOPS")
                .map_or(1_000_000, |env_var| env_var.parse().unwrap());
            debug_assert!(state.clk != limit, "Looped for longer than MOZAK_MAX_LOOPS");
        }
    }
    Ok((rows, state))
}
}

#[cfg(test)]
mod tests {
    use alloc::collections::BTreeMap;

    use anyhow::Result;
    use test_case::test_case;

    use crate::{elf::Program, state::State};
    impl State {
        pub fn set_register_value_mut(&mut self, index: usize, value: u32) {
            *self = self.clone().set_register_value(index, value);
        }

        pub fn set_pc_mut(&mut self, value: u32) {
            *self = self.clone().set_pc(value);
        }

        #[must_use]
        pub fn get_register_value_signed(&self, index: usize) -> i32 {
            self.get_register_value(index) as i32
        }

        /// Store a word to memory
        ///
        /// # Errors
        /// This function returns an error, if you try to store to an invalid
        /// address.
        pub fn store_u32(&mut self, addr: u32, value: u32) -> Result<()> {
            let bytes = value.to_le_bytes();
            for (i, byte) in bytes.iter().enumerate() {
                *self = self.clone().store_u8(addr + i as u32, *byte);
            }
            Ok(())
        }

        /// Load a halfword from memory
        ///
        /// # Errors
        /// This function returns an error, if you try to load from an invalid
        /// address.
        #[must_use]
        pub fn load_u16(&self, addr: u32) -> u16 {
            let mut bytes = [0_u8; 2];
            bytes[0] = self.load_u8(addr);
            bytes[1] = self.load_u8(addr + 1_u32);
            u16::from_le_bytes(bytes)
        }
    }

    fn create_prog(image: BTreeMap<u32, u32>) -> State {
        State::from(&Program::from(image))
    }

    fn simple_test(exit_at: u32, mem: &[(u32, u32)], regs: &[(usize, u32)]) -> State {
        // TODO(Matthias): stick this line into proper common setup?
        let _ = env_logger::try_init();
        let exit_inst =
              // set sys-call EXIT in x17(or a7)
              &[(exit_at, 0x05d0_0893_u32),
              // add ECALL to halt the program
              (exit_at + 4, 0x0000_0073_u32)];

        let image: BTreeMap<u32, u32> = mem.iter().chain(exit_inst.iter()).copied().collect();

        let state = regs.iter().fold(create_prog(image), |state, (rs, val)| {
            state.set_register_value(*rs, *val)
        });

        let state = Vm::step(state).unwrap().1;
        assert!(state.has_halted());
        state
    }

    // NOTE: For writing test cases please follow RISCV
    // calling convention for using registers in instructions.
    // Please check https://en.wikichip.org/wiki/risc-v/registers

    #[test_case(0x0073_02b3, 5, 6, 7, 60049, 50493; "add r5, r6, r7")]
    #[test_case(0x01FF_8FB3, 31, 31, 31, 8981, 8981; "add r31, r31, r31")]
    fn add(word: u32, rd: usize, rs1: usize, rs2: usize, rs1_value: u32, rs2_value: u32) {
        let state = simple_test(4, &[(0_u32, word)], &[(rs1, rs1_value), (rs2, rs2_value)]);
        assert_eq!(state.get_register_value(rd), rs1_value + rs2_value);
    }

    // Tests 2 cases:
    //   1) rs2 overflow (0x1111 should only use lower 5 bits)
    //   2) rs1 overflow (0x12345678 << 0x08 == 0x34567800)
    #[test_case(0x0073_12b3, 5, 6, 7, 7, 0x1111; "sll r5, r6, r7, only lower 5 bits rs2")]
    #[test_case(0x0139_12b3, 5, 18, 19, 0x1234_5678, 0x08; "sll r5, r18, r19, rs1 overflow")]
    fn sll(word: u32, rd: usize, rs1: usize, rs2: usize, rs1_value: u32, rs2_value: u32) {
        let state = simple_test(4, &[(0_u32, word)], &[(rs1, rs1_value), (rs2, rs2_value)]);
        assert_eq!(
            state.get_register_value(rd),
            rs1_value << (rs2_value & 0x1F)
        );
    }

    #[test_case(0x0073_72b3, 5, 6, 7, 7, 8; "and r5, r6, r7")]
    fn and(word: u32, rd: usize, rs1: usize, rs2: usize, rs1_value: u32, rs2_value: u32) {
        let state = simple_test(4, &[(0_u32, word)], &[(rs1, rs1_value), (rs2, rs2_value)]);
        assert_eq!(state.get_register_value(rd), rs1_value & rs2_value);
    }

    // Tests 2 cases:
    //   1) rs2 overflow (0x1111 should only use lower 5 bits)
    //   2) rs1 underflow (0x87654321 >> 0x08 == 0x00876543)
    #[test_case(0x0073_52b3, 5, 6, 7, 7, 0x1111; "srl r5, r6, r7, only lower 5 bits rs2")]
    #[test_case(0x0139_52b3, 5, 18, 19, 0x8765_4321, 0x08; "srl r5, r18, r19, rs1 underflow")]
    fn srl(word: u32, rd: usize, rs1: usize, rs2: usize, rs1_value: u32, rs2_value: u32) {
        let state = simple_test(4, &[(0_u32, word)], &[(rs1, rs1_value), (rs2, rs2_value)]);
        assert_eq!(
            state.get_register_value(rd),
            rs1_value >> (rs2_value & 0x1F)
        );
    }

    #[test_case(0x0073_62b3, 5, 6, 7, 7, 8; "or r5, r6, r7")]
    fn or(word: u32, rd: usize, rs1: usize, rs2: usize, rs1_value: u32, rs2_value: u32) {
        let state = simple_test(4, &[(0_u32, word)], &[(rs1, rs1_value), (rs2, rs2_value)]);
        assert_eq!(state.get_register_value(rd), rs1_value | rs2_value);
    }

    // Tests 2 cases:
    //   1) x6 = 0x55551111, imm = 0xff (255), x5 = 0x555511ff
    //   2) x6 = 0x55551111, imm = 0x800 (-2048), x5 = 0xfffff911
    #[test_case(0x0ff3_6293, 5, 6, 0x5555_1111, 255; "ori r5, r6, 255")]
    #[test_case(0x8003_6293, 5, 6, 0x5555_1111, -2048; "ori r5, r6, -2048")]
    fn ori(word: u32, rd: usize, rs1: usize, rs1_value: u32, imm: i32) {
        let state = simple_test(4, &[(0_u32, word)], &[(rs1, rs1_value)]);

        let expected_value = (rs1_value as i32 | imm) as u32;
        assert_eq!(state.get_register_value(rd), expected_value);
    }

    // Tests 2 cases:
    //   1) x6 = 0x55551111, imm = 0xff (255), x5 = 0x555510000
    //   2) x6 = 0x55551111, imm = 0x800 (-2048), x5 = 0x00000011
    #[test_case(0x0ff3_7293, 5, 6, 0x5555_1111, 255; "andi r5, r6, 255")]
    #[test_case(0x8003_7293, 5, 6, 0x5555_1111, -2048; "andi r5, r6, -2048")]
    fn andi(word: u32, rd: usize, rs1: usize, rs1_value: u32, imm: i32) {
        let state = simple_test(4, &[(0_u32, word)], &[(rs1, rs1_value)]);
        let expected_value = (rs1_value as i32 & imm) as u32;
        assert_eq!(state.get_register_value(rd), expected_value);
    }

    #[test_case(0x0073_42b3, 5, 6, 7, 0x0000_1111, 0x0011_0011; "xor r5, r6, r7")]
    fn xor(word: u32, rd: usize, rs1: usize, rs2: usize, rs1_value: u32, rs2_value: u32) {
        let state = simple_test(4, &[(0_u32, word)], &[(rs1, rs1_value), (rs2, rs2_value)]);

        let expected_value = rs1_value ^ rs2_value;
        assert_eq!(state.get_register_value(rd), expected_value);
    }

    // Tests 2 cases:
    //   1) x6 = 0x55551111, imm = 0xff (255), x5 = 0x555511ff
    //   2) x6 = 0x55551111, imm = 0x800 (-2048), x5 = 0xfffff911
    #[test_case(0x0ff3_4293, 5, 6, 0x5555_1111, 255; "xori r5, r6, 255")]
    #[test_case(0x8003_4293, 5, 6, 0x5555_1111, -2048; "xori r5, r6, -2048")]
    fn xori(word: u32, rd: usize, rs1: usize, rs1_value: u32, imm: i32) {
        let state = simple_test(4, &[(0_u32, word)], &[(rs1, rs1_value)]);

        let expected_value = (rs1_value as i32 ^ imm) as u32;
        assert_eq!(state.get_register_value(rd), expected_value);
    }

    // Tests 2 cases:
    //   1) rs2 overflow (0x1111 should only use lower 5 bits)
    //   2) rs1 underflow (0x87654321 >> 0x08 == 0xff876543)
    #[test_case(0x4073_52b3, 5, 6, 7, 7, 0x1111; "sra r5, r6, r7, only lower 5 bits rs2")]
    #[test_case(0x4139_52b3, 5, 18, 19, 0x8765_4321, 0x08; "sra r5, r18, r19, rs1 underflow")]
    fn sra(word: u32, rd: usize, rs1: usize, rs2: usize, rs1_value: u32, rs2_value: u32) {
        let state = simple_test(4, &[(0_u32, word)], &[(rs1, rs1_value), (rs2, rs2_value)]);
        assert_eq!(
            state.get_register_value(rd),
            (rs1_value as i32 >> (rs2_value & 0x1F) as i32) as u32
        );
    }

    // x6 = 0x8000ffff x7 = 0x12345678, x5 = 0x00000001
    // x6 = 0x12345678 x7 = 0x8000ffff, x5 = 0x00000000
    // x6 = 0x12345678 x7 = 0x0000ffff, x5 = 0x00000000
    // x18 = 0x82345678 x19 = 0x8000ffff, x5 = 0x00000001
    #[test_case(0x0073_22b3, 5, 6, 7, 0x8000_ffff, 0x1234_5678; "slt r5, r6, r7, neg rs1")]
    #[test_case(0x0073_22b3, 5, 6, 7, 0x1234_5678, 0x8000_ffff; "slt r5, r6, r7, neg rs2")]
    #[test_case(0x0073_22b3, 5, 6, 7, 0x1234_5678, 0x0000_ffff; "slt r5, r6, r7")]
    #[test_case(0x0139_22b3, 5, 18, 19, 0x8234_5678, 0x0000_ffff; "slt r5, r18, r19")]
    fn slt(word: u32, rd: usize, rs1: usize, rs2: usize, rs1_value: u32, rs2_value: u32) {
        let state = simple_test(4, &[(0_u32, word)], &[(rs1, rs1_value), (rs2, rs2_value)]);
        let rs1_value = rs1_value as i32;
        let rs2_value = rs2_value as i32;
        assert_eq!(
            state.get_register_value(rd),
            u32::from(rs1_value < rs2_value)
        );
    }

    #[test_case(0x4043_5293, 5, 6, 0x8765_4321, 4; "srai r5, r6, 4")]
    #[test_case(0x41f3_5293, 5, 6, 1, 31; "srai r5, r6, 31")]
    fn srai(word: u32, rd: usize, rs1: usize, rs1_value: u32, imm: i32) {
        let state = simple_test(4, &[(0_u32, word)], &[(rs1, rs1_value)]);
        assert_eq!(
            state.get_register_value(rd),
            (rs1_value as i32 >> imm) as u32
        );
    }

    #[test_case(0x0043_5293, 5, 6, 0x8765_4321, 4; "srli r5, r6, 4")]
    #[test_case(0x01f3_5293, 5, 6, 1, 31; "srli r5, r6, 31")]
    fn srli(word: u32, rd: usize, rs1: usize, rs1_value: u32, imm: i32) {
        let state = simple_test(4, &[(0_u32, word)], &[(rs1, rs1_value)]);
        assert_eq!(state.get_register_value(rd), rs1_value >> imm);
    }

    #[test_case(0x0043_1293, 5, 6, 0x8765_4321, 4; "slli r5, r6, 4")]
    #[test_case(0x01f3_1293, 5, 6, 1, 31; "slli r5, r6, 31")]
    fn slli(word: u32, rd: usize, rs1: usize, rs1_value: u32, imm: i32) {
        let state = simple_test(4, &[(0_u32, word)], &[(rs1, rs1_value)]);
        assert_eq!(state.get_register_value(rd), rs1_value << imm);
    }

    #[test_case(0x8009_2293, 5, 6, 1, -2048; "slti r5, r6, -2048")]
    #[test_case(0xfff3_2293, 5, 6, 1, -1; "slti r5, r6, -1")]
    #[test_case(0x0009_2293, 5, 6, 1, 0; "slti r5, r6, 0")]
    #[test_case(0x7ff3_2293, 5, 6, 1, 2047; "slti r5, r6, 2047")]
    fn slti(word: u32, rd: usize, rs1: usize, rs1_value: u32, imm: i32) {
        let state = simple_test(4, &[(0_u32, word)], &[(rs1, rs1_value)]);
        let rs1_value = rs1_value as i32;
        assert_eq!(state.get_register_value(rd), u32::from(rs1_value < imm));
    }

    #[test_case(0x8003_3293, 5, 6, 1, -2048; "sltiu r5, r6, -2048")]
    #[test_case(0xfff3_3293, 5, 6, 1, -1; "sltiu r5, r6, -1")]
    #[test_case(0x0003_3293, 5, 6, 1, 0; "sltiu r5, r6, 0")]
    #[test_case(0x7ff3_3293, 5, 6, 1, 2047; "sltiu r5, r6, 2047")]
    fn sltiu(word: u32, rd: usize, rs1: usize, rs1_value: u32, imm: i32) {
        let state = simple_test(4, &[(0_u32, word)], &[(rs1, rs1_value)]);
        assert_eq!(
            state.get_register_value(rd),
            u32::from(rs1_value < imm as u32)
        );
    }

    // x6 = 0x12345678 x7 = 0x0000ffff, x5 = 0x00000000
    // x18 = 0x12345678 x19 = 0x8000ffff, x5 = 0x00000001
    #[test_case(0x0073_32b3, 5, 6, 7, 0x1234_5678, 0x0000_ffff; "sltu r5, r6, r7")]
    #[test_case(0x0139_32b3, 5, 18, 19, 0x1234_5678, 0x8000_ffff; "sltu r5, r18, r19")]
    fn sltu(word: u32, rd: usize, rs1: usize, rs2: usize, rs1_value: u32, rs2_value: u32) {
        let state = simple_test(4, &[(0_u32, word)], &[(rs1, rs1_value), (rs2, rs2_value)]);
        assert_eq!(
            state.get_register_value(rd),
            u32::from(rs1_value < rs2_value)
        );
    }

    #[test_case(0x05d0_0393, 7, 0, 0, 93; "addi r7, r0, 93")]
    fn addi(word: u32, rd: usize, rs1: usize, rs1_value: u32, imm: i32) {
        let state = simple_test(4, &[(0_u32, word)], &[(rs1, rs1_value)]);
        let mut expected_value = rs1_value;
        if imm.is_negative() {
            expected_value -= imm.unsigned_abs();
        } else {
            expected_value += imm as u32;
        }
        assert_eq!(state.get_register_value(rd), expected_value);
    }

    #[test_case(0x0643_0283, 5, 6, 100, 0, 127; "lb r5, 100(r6)")]
    #[test_case(0x0643_0283, 5, 6, 100, 200, 127; "lb r5, -100(r6) offset_negative")]
    #[test_case(0x0643_0283, 5, 6, 100, 0, -128; "lb r5, 100(r6) value_negative")]
    #[test_case(0x0643_0283, 5, 6, 100, 200, -128; "lb r5, -100(r6) offset_negative_value_negative")]
    fn lb(word: u32, rd: usize, rs1: usize, offset: i16, rs1_value: u32, memory_value: i8) {
        let mut address: u32 = rs1_value;
        if offset.is_negative() {
            let abs_offset = u32::from(offset.unsigned_abs());
            assert!(abs_offset <= rs1_value);
            address -= u32::from(offset.unsigned_abs());
        } else {
            address += offset as u32;
        }
        let state = simple_test(
            4,
            &[(0_u32, word), (address, memory_value as u32)],
            &[(rs1, rs1_value)],
        );
        let mut expected_value = memory_value as u32;
        if memory_value.is_negative() {
            // extend the sign
            expected_value |= 0xffff_ff00;
        }
        assert_eq!(state.get_register_value(rd), expected_value);
    }

    #[test_case(0x0643_4283, 5, 6, 100, 0, 127; "lbu r5, 100(r6)")]
    #[test_case(0x0643_4283, 5, 6, 100, 200, 127; "lbu r5, -100(r6) offset_negative")]
    #[test_case(0x0643_4283, 5, 6, 100, 0, -128; "lbu r5, 100(r6) value_negative")]
    #[test_case(0x0643_4283, 5, 6, 100, 200, -128; "lbu r5, -100(r6) offset_negative_value_negative")]
    fn lbu(word: u32, rd: usize, rs1: usize, offset: i16, rs1_value: u32, memory_value: i8) {
        let mut address: u32 = rs1_value;
        if offset.is_negative() {
            let abs_offset = u32::from(offset.unsigned_abs());
            assert!(abs_offset <= rs1_value);
            address -= u32::from(offset.unsigned_abs());
        } else {
            address += offset as u32;
        }
        let state = simple_test(
            4,
            &[(0_u32, word), (address, memory_value as u32)],
            &[(rs1, rs1_value)],
        );
        let expected_value = (memory_value as u32) & 0x0000_00FF;
        assert_eq!(state.get_register_value(rd), expected_value);
    }

    #[test_case(0x0643_1283, 5, 6, 100, 0, 4096; "lh r5, 100(r6)")]
    #[test_case(0x0643_1283, 5, 6, 100, 200, 4096; "lh r5, -100(r6) offset_negative")]
    #[test_case(0x0643_1283, 5, 6, 100, 0, -4095; "lh r5, 100(r6) value_negative")]
    #[test_case(0x0643_1283, 5, 6, 100, 200, -4095; "lh r5, -100(r6) offset_negative_value_negative")]
    fn lh(word: u32, rd: usize, rs1: usize, offset: i16, rs1_value: u32, memory_value: i16) {
        let mut address: u32 = rs1_value;
        if offset.is_negative() {
            let abs_offset = u32::from(offset.unsigned_abs());
            assert!(abs_offset <= rs1_value);
            address -= u32::from(offset.unsigned_abs());
        } else {
            address += offset as u32;
        }
        let state = simple_test(
            4,
            &[(0_u32, word), (address, memory_value as u32)],
            &[(rs1, rs1_value)],
        );
        let mut expected_value = memory_value as u32;
        if memory_value.is_negative() {
            // extend the sign
            expected_value |= 0xffff_0000;
        }
        assert_eq!(state.get_register_value(rd), expected_value);
    }

    #[test_case(0x0643_5283, 5, 6, 100, 0, 4096; "lhu r5, 100(r6)")]
    #[test_case(0x0643_5283, 5, 6, 100, 200, 4096; "lhu r5, -100(r6) offset_negative")]
    #[test_case(0x0643_5283, 5, 6, 100, 0, -4095; "lhu r5, 100(r6) value_negative")]
    #[test_case(0x0643_5283, 5, 6, 100, 200, -4095; "lhu r5, -100(r6) offset_negative_value_negative")]
    fn lhu(word: u32, rd: usize, rs1: usize, offset: i16, rs1_value: u32, memory_value: i16) {
        let mut address: u32 = rs1_value;
        if offset.is_negative() {
            let abs_offset = u32::from(offset.unsigned_abs());
            assert!(abs_offset <= rs1_value);
            address -= u32::from(offset.unsigned_abs());
        } else {
            address += offset as u32;
        }
        let state = simple_test(
            4,
            &[(0_u32, word), (address, memory_value as u32)],
            &[(rs1, rs1_value)],
        );
        let expected_value = (memory_value as u32) & 0x0000_FFFF;
        assert_eq!(state.get_register_value(rd), expected_value);
    }

    #[test_case(0x0643_2283, 5, 6, 100, 0, 65535; "lw r5, 100(r6)")]
    #[test_case(0x0643_2283, 5, 6, 100, 200, 65535; "lw r5, -100(r6) offset_negative")]
    #[test_case(0x0643_2283, 5, 6, 100, 0, -65535; "lw r5, 100(r6) value_negative")]
    #[test_case(0x0643_2283, 5, 6, 100, 200, -65535; "lw r5, -100(r6) offset_negative_value_negative")]
    fn lw(word: u32, rd: usize, rs1: usize, offset: i16, rs1_value: u32, memory_value: i32) {
        let mut address: u32 = rs1_value;
        if offset.is_negative() {
            let abs_offset = u32::from(offset.unsigned_abs());
            assert!(abs_offset <= rs1_value);
            address -= u32::from(offset.unsigned_abs());
        } else {
            address += offset as u32;
        }
        let state = simple_test(
            4,
            &[(0_u32, word), (address, memory_value as u32)],
            &[(rs1, rs1_value)],
        );
        let expected_value = memory_value as u32;
        assert_eq!(state.get_register_value(rd), expected_value);
    }

    // TODO: Add more tests for JAL/JALR
    #[test]
    fn jal_jalr() {
        let _ = env_logger::try_init();
        let mem =
        // at 0 address instruction jal to 256
        // JAL x1, 256
        [(0_u32, 0x1000_00ef),
        // set R5 to 100 so that it can be verified
        // that indeed control passed to this location
        // ADDI x5, x0, 100
            (256_u32, 0x0640_0293),
        // at 260 go back to address after JAL
        // JALR x0, x1, 0
            (260_u32, 0x0000_8067)];
        let state = simple_test(4, &mem, &[]);
        assert_eq!(state.get_register_value(5_usize), 100_u32);
    }

    #[test]
    fn jalr_same_registers() {
        let mem = [
            // at 0 address instruction jal to 256
            // JAL x1, 256
            (0_u32, 0x1000_00ef),
            // set R5 to 100 so that it can be verified
            // that indeed control passed to this location
            // ADDI x5, x0, 100
            (256_u32, 0x0640_0293),
            // at 260 go back to address after JAL
            // JALR x1, x1, 0
            (260_u32, 0x0000_80e7),
        ];
        let state = simple_test(4, &mem, &[]);

        assert_eq!(state.get_register_value(5_usize), 100_u32);
        // JALR at 260 updates X1 to have value of next_pc i.e 264
        assert_eq!(state.get_register_value(1_usize), 264_u32);
    }

    #[test]
    fn beq() {
        let mem = [
            // at 0 address instruction BEQ to 256
            // BEQ x0, x1, 256
            (0_u32, 0x1010_0063),
            // set R5 to 100 so that it can be verified
            // that indeed control passed to this location
            // ADDI x5, x0, 100
            (256_u32, 0x0640_0293),
            // at 260 go back to address after BEQ
            // JAL x0, -256
            (260_u32, 0xf01f_f06f),
        ];
        let state = simple_test(4, &mem, &[]);

        assert_eq!(state.get_register_value(5_usize), 100_u32);
    }

    #[test]
    fn bne() {
        let mem = [
            // at 0 address instruction BNE to 256
            // BNE x0, x1, 256
            (0_u32, 0x1010_1063),
            // set R5 to 100 so that it can be verified
            // that indeed control passed to this location
            // ADDI x5, x0, 100
            (256_u32, 0x0640_0293),
            // at 260 go back to address after BNE
            // JAL x0, -256
            (260_u32, 0xf01f_f06f),
        ];
        let state = simple_test(4, &mem, &[(1, 1)]);

        assert_eq!(state.get_register_value(5_usize), 100_u32);
    }

    #[test]
    fn blt() {
        let mem = [
            // at 0 address instruction BLT to 256
            // BLT x1, x0, 256
            (0_u32, 0x1000_c063),
            // set R5 to 100 so that it can be verified
            // that indeed control passed to this location
            // ADDI x5, x0, 100
            (256_u32, 0x0640_0293),
            // at 260 go back to address after BLT
            // JAL x0, -256
            (260_u32, 0xf01f_f06f),
        ];

        // set R1 = -1
        let state = simple_test(4, &mem, &[(1, 0xffff_ffff)]);

        assert_eq!(state.get_register_value(5_usize), 100_u32);
    }

    #[test]
    fn bltu() {
        let mem = [
            // at 0 address instruction BLTU to 256
            // BLTU x1, x2, 256
            (0_u32, 0x1020_e063),
            // set R5 to 100 so that it can be verified
            // that indeed control passed to this location
            // ADDI x5, x0, 100
            (256_u32, 0x0640_0293),
            // at 260 go back to address after BLTU
            // JAL x0, -256
            (260_u32, 0xf01f_f06f),
        ];
        let state = simple_test(4, &mem, &[(1_usize, 0xffff_fffe), (2_usize, 0xffff_ffff)]);

        assert_eq!(state.get_register_value(5_usize), 100_u32);
    }

    #[test]
    fn bge() {
        let mem = [
            // at 0 address instruction BGE to 256
            // BGE x0, x1, 256
            (0_u32, 0x1010_5063),
            // set R5 to 100 so that it can be verified
            // that indeed control passed to this location
            // ADDI x5, x0, 100
            (256_u32, 0x0640_0293),
            // at 260 go back to address after BGE
            // JAL x0, -256
            (260_u32, 0xf01f_f06f),
        ];
        // set R1 = -1
        let state = simple_test(4, &mem, &[(1_usize, 0xffff_ffff)]);

        assert_eq!(state.get_register_value(5_usize), 100_u32);
    }

    #[test]
    fn bgeu() {
        let mem = [
            // at 0 address instruction BGEU to 256
            // BGEU x2, x1, 256
            (0_u32, 0x1011_7063),
            // set R5 to 100 so that it can be verified
            // that indeed control passed to this location
            // ADDI x5, x0, 100
            (256_u32, 0x0640_0293),
            // at 260 go back to address after BGEU
            // JAL x0, -256
            (260_u32, 0xf01f_f06f),
        ];
        let state = simple_test(4, &mem, &[(1_usize, 0xffff_fffe), (2_usize, 0xffff_ffff)]);

        assert_eq!(state.get_register_value(5_usize), 100_u32);
    }

    #[test]
    fn sb() {
        // at 0 address instruction SB
        // SB x5, 1200(x0)
        let state = simple_test(4, &[(0, 0x4a50_0823)], &[(5, 0x0000_00FF)]);

        assert_eq!(state.load_u32(1200), 0x0000_00FF);
    }

    #[test]
    fn sh() {
        // at 0 address instruction SH
        // SH x5, 1200(x0)
        let state = simple_test(4, &[(0, 0x4a50_1823)], &[(5_usize, 0x0000_BABE)]);
        // assert_eq!(vm.state.load_u32(1200), 0);

        assert_eq!(state.load_u32(1200), 0x0000_BABE);
    }

    #[test]
    fn sw() {
        // at 0 address instruction SW
        // SW x5, 1200(x0)
        let state = simple_test(4, &[(0, 0x4a50_2823)], &[(5_usize, 0xC0DE_BABE)]);
        // assert_eq!(vm.state.load_u32(1200), 0);

        assert_eq!(state.load_u32(1200), 0xC0DE_BABE);
    }

    #[test]
    fn mulh() {
        // at 0 address instruction MULH
        // MULH x5, x6, x7
        let state = simple_test(
            4,
            &[(0, 0x0273_12b3)],
            &[
                (6_usize, 0x8000_0000 /* == -2^31 */),
                (7_usize, 0x8000_0000 /* == -2^31 */),
            ],
        );

        assert_eq!(
            state.get_register_value(5_usize),
            0x4000_0000 // High bits for 2^62
        );
    }

    #[test]
    fn mul() {
        // at 0 address instruction MUL
        // MUL x5, x6, x7
        let state = simple_test(
            4,
            &[(0, 0x0273_02b3)],
            &[
                (6_usize, 0x4000_0000 /* == 2^30 */),
                (7_usize, 0xFFFF_FFFE /* == -2 */),
            ],
        );
        assert_eq!(
            state.get_register_value(5_usize),
            0x8000_0000 // -2^31
        );
    }

    #[test]
    fn mulhsu() {
        // at 0 address instruction MULHSU
        // MULHSU x5, x6, x7
        let state = simple_test(
            4,
            &[(0_u32, 0x0273_22b3)],
            &[
                (6_usize, 0xFFFF_FFFE /* == -2 */),
                (7_usize, 0x4000_0000 /* == 2^30 */),
            ],
        );
        assert_eq!(state.get_register_value(5_usize), 0xFFFF_FFFF);
    }

    #[test]
    fn mulhu() {
        // at 0 address instruction MULHU
        // MULHU x5, x6, x7
        let state = simple_test(
            4,
            &[(0_u32, 0x0273_32b3)],
            &[
                (6_usize, 0x0000_0002 /* == 2 */),
                (7_usize, 0x8000_0000 /* == 2^31 */),
            ],
        );
        assert_eq!(state.get_register_value(5_usize), 0x0000_0001);
    }

    #[test]
    fn lui() {
        // at 0 address instruction lui
        // LUI x1, -524288
        let state = simple_test(4, &[(0_u32, 0x8000_00b7)], &[]);
        assert_eq!(state.get_register_value(1), 0x8000_0000);
        assert_eq!(state.get_register_value_signed(1), -2_147_483_648);
    }

    #[test]
    fn auipc() {
        // at 0 address addi x0, x0, 0
        let state = simple_test(
            8,
            &[
                (0_u32, 0x0000_0013),
                // at 4 address instruction auipc
                // auipc x1, -524288
                (4_u32, 0x8000_0097),
            ],
            &[],
        );
        assert_eq!(state.get_register_value(1), 0x8000_0004);
        assert_eq!(state.get_register_value_signed(1), -2_147_483_644);
    }

    #[test]
    fn system_opcode_instructions() {
        simple_test(
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

    #[test_case(0x4000_0000 /*2^30*/, 0xFFFF_FFFE /*-2*/, 0xE000_0000 /*-2^29*/; "simple")]
    #[test_case(0x4000_0000, 0x0000_0000, 0xFFFF_FFFF; "div_by_zero")]
    #[test_case(0x8000_0000 /*-2^31*/, 0xFFFF_FFFF /*-1*/, 0x8000_0000; "overflow")]
    fn div(rs1_value: u32, rs2_value: u32, rd_value: u32) {
        // at 0 address instruction DIV
        // DIV x5, x6, x7
        let state = simple_test(
            4,
            &[(0_u32, 0x0273_42b3)],
            &[
                (6_usize, rs1_value /* == 2^30 */),
                (7_usize, rs2_value /* == -2 */),
            ],
        );
        assert_eq!(
            state.get_register_value(5_usize),
            rd_value // -2^29
        );
    }

    #[test_case(0x8000_0000 /*2^31*/, 0x0000_0002 /*2*/, 0x4000_0000 /*2^30*/; "simple")]
    #[test_case(0x4000_0000, 0x0000_0000, 0xFFFF_FFFF; "div_by_zero")]
    fn divu(rs1_value: u32, rs2_value: u32, rd_value: u32) {
        // at 0 address instruction DIVU
        // DIVU x5, x6, x7
        let state = simple_test(
            4,
            &[(0_u32, 0x0273_52b3)],
            &[(6_usize, rs1_value), (7_usize, rs2_value)],
        );
        assert_eq!(state.get_register_value(5_usize), rd_value);
    }

    #[test_case(0xBFFF_FFFD /*-2^31 - 3*/, 0x0000_0002 /*2*/, 0xFFFF_FFFF /*-1*/; "simple")]
    #[test_case(0x4000_0000, 0x0000_0000, 0x4000_0000; "div_by_zero")]
    #[test_case(0x8000_0000 /*-2^31*/, 0xFFFF_FFFF /*-1*/, 0x0000_0000; "overflow")]
    fn rem(rs1_value: u32, rs2_value: u32, rd_value: u32) {
        // at 0 address instruction REM
        // REM x5, x6, x7
        let state = simple_test(
            4,
            &[(0_u32, 0x0273_62b3)],
            &[(6_usize, rs1_value), (7_usize, rs2_value)],
        );
        assert_eq!(state.get_register_value(5_usize), rd_value);
    }

    #[test_case(0x8000_0003 /*2^31 + 3*/, 0x0000_0002 /*2*/, 0x000_0001 /*1*/; "simple")]
    #[test_case(0x4000_0000, 0x0000_0000, 0x4000_0000; "div_by_zero")]
    fn remu(rs1_value: u32, rs2_value: u32, rd_value: u32) {
        // at 0 address instruction REMU
        // REMU x5, x6, x7
        let state = simple_test(
            4,
            &[(0_u32, 0x0273_72b3)],
            &[(6_usize, rs1_value), (7_usize, rs2_value)],
        );
        assert_eq!(state.get_register_value(5_usize), rd_value);
    }
}

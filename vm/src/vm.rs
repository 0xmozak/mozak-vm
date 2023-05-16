use anyhow::Result;
use log::trace;

use crate::{decode::decode_instruction, instruction::Instruction, state::State};

pub struct Vm {
    pub state: State,
}

impl Vm {
    pub fn new(state: State) -> Self {
        Self { state }
    }

    pub fn step(&mut self) -> Result<()> {
        while !self.state.has_halted() {
            let pc = self.state.get_pc();
            let word = self.state.load_u32(pc)?;
            let inst = decode_instruction(word);
            trace!("Decoded Inst: {:?}", inst);
            self.execute_instruction(&inst)?;
        }
        Ok(())
    }

    fn execute_instruction(&mut self, inst: &Instruction) -> Result<()> {
        match inst {
            Instruction::ADD(add) => {
                // TODO: how to handle if regs have negative value?
                let res = self.state.get_register_value(add.rs1.into())
                    + self.state.get_register_value(add.rs2.into());
                self.state.set_register_value(add.rd.into(), res);
                self.state.set_pc(self.state.get_pc() + 4);
                Ok(())
            }
            Instruction::SLL(sll) => {
                let res = self.state.get_register_value(sll.rs1.into())
                    << self.state.get_register_value(sll.rs2.into());
                self.state.set_register_value(sll.rd.into(), res);
                self.state.set_pc(self.state.get_pc() + 4);
                Ok(())
            }
            Instruction::SRL(srl) => {
                let res = self.state.get_register_value(srl.rs1.into())
                    >> self.state.get_register_value(srl.rs2.into());
                self.state.set_register_value(srl.rd.into(), res);
                self.state.set_pc(self.state.get_pc() + 4);
                Ok(())
            }
            Instruction::SRA(sra) => {
                let res = self.state.get_register_value_signed(sra.rs1.into())
                    >> self.state.get_register_value_signed(sra.rs2.into());
                self.state.set_register_value(sra.rd.into(), res as u32);
                self.state.set_pc(self.state.get_pc() + 4);
                Ok(())
            }
            Instruction::SLT(slt) => {
                let res = self.state.get_register_value_signed(slt.rs1.into())
                    < self.state.get_register_value_signed(slt.rs2.into());
                self.state.set_register_value(slt.rd.into(), res.into());
                self.state.set_pc(self.state.get_pc() + 4);
                Ok(())
            }
            Instruction::SLTU(sltu) => {
                let res = self.state.get_register_value(sltu.rs1.into())
                    < self.state.get_register_value(sltu.rs2.into());
                self.state.set_register_value(sltu.rd.into(), res.into());
                self.state.set_pc(self.state.get_pc() + 4);
                Ok(())
            }
            Instruction::ADDI(addi) => {
                // TODO: how to handle if regs have negative value?
                let rs1_value: i64 = self.state.get_register_value(addi.rs1.into()).into();
                let mut res = rs1_value;
                if addi.imm12.is_negative() {
                    res -= addi.imm12 as i64;
                } else {
                    res += addi.imm12 as i64;
                }
                // ignore anything above 32-bits
                let res: u32 = (res & 0xffffffff) as u32;
                self.state.set_register_value(addi.rd.into(), res);
                self.state.set_pc(self.state.get_pc() + 4);
                Ok(())
            }
            Instruction::SUB(sub) => {
                let res = self.state.get_register_value(sub.rs1.into())
                    - self.state.get_register_value(sub.rs2.into());
                self.state.set_register_value(sub.rd.into(), res);
                self.state.set_pc(self.state.get_pc() + 4);
                Ok(())
            }
            Instruction::LB(load) => {
                let rs1: i64 = self.state.get_register_value(load.rs1.into()).into();
                let addr = rs1 + load.imm12 as i64;
                let addr: u32 = (addr & 0xffffffff) as u32;
                let value: u8 = self.state.load_u8(addr)?;
                let mut final_value: u32 = value.into();
                if value & 0x80 != 0x0 {
                    // extend sign bit
                    final_value |= 0xffffff00;
                }
                self.state.set_register_value(load.rd.into(), final_value);
                self.state.set_pc(self.state.get_pc() + 4);
                Ok(())
            }
            Instruction::ECALL => {
                let r17_value = self.state.get_register_value(17_usize);
                #[allow(clippy::single_match)]
                match r17_value {
                    93 => {
                        // exit system call
                        self.state.halt();
                    }
                    _ => {}
                }
                Ok(())
            }
            Instruction::JAL(jal) => {
                let pc = self.state.get_pc();
                let next_pc = pc + 4;
                self.state.set_register_value(jal.rd.into(), next_pc);
                let jump_pc = (pc as i32) + jal.imm20;
                self.state.set_pc(jump_pc as u32);
                Ok(())
            }
            Instruction::JALR(jalr) => {
                let pc = self.state.get_pc();
                let next_pc = pc + 4;
                self.state.set_register_value(jalr.rd.into(), next_pc);
                let rs1_value = self.state.get_register_value(jalr.rs1.into());
                let jump_pc = (rs1_value as i32) + jalr.imm12 as i32;
                self.state.set_pc(jump_pc as u32);
                Ok(())
            }
            Instruction::BEQ(beq) => {
                if self.state.get_register_value(beq.rs1.into())
                    == self.state.get_register_value(beq.rs2.into())
                {
                    let pc = self.state.get_pc();
                    let jump_pc = (pc as i32) + beq.imm12 as i32;
                    self.state.set_pc(jump_pc as u32);
                } else {
                    self.state.set_pc(self.state.get_pc() + 4);
                }
                Ok(())
            }
            Instruction::BNE(bne) => {
                if self.state.get_register_value(bne.rs1.into())
                    != self.state.get_register_value(bne.rs2.into())
                {
                    let pc = self.state.get_pc();
                    let jump_pc = (pc as i32) + bne.imm12 as i32;
                    self.state.set_pc(jump_pc as u32);
                } else {
                    self.state.set_pc(self.state.get_pc() + 4);
                }
                Ok(())
            }
            Instruction::BLT(blt) => {
                if self.state.get_register_value_signed(blt.rs1.into())
                    < self.state.get_register_value_signed(blt.rs2.into())
                {
                    let pc = self.state.get_pc();
                    let jump_pc = (pc as i32) + blt.imm12 as i32;
                    self.state.set_pc(jump_pc as u32);
                } else {
                    self.state.set_pc(self.state.get_pc() + 4);
                }
                Ok(())
            }
            Instruction::BLTU(bltu) => {
                if self.state.get_register_value(bltu.rs1.into())
                    < self.state.get_register_value(bltu.rs2.into())
                {
                    let pc = self.state.get_pc();
                    let jump_pc = (pc as i32) + bltu.imm12 as i32;
                    self.state.set_pc(jump_pc as u32);
                } else {
                    self.state.set_pc(self.state.get_pc() + 4);
                }
                Ok(())
            }
            Instruction::BGE(bge) => {
                if self.state.get_register_value_signed(bge.rs1.into())
                    >= self.state.get_register_value_signed(bge.rs2.into())
                {
                    let pc = self.state.get_pc();
                    let jump_pc = (pc as i32) + bge.imm12 as i32;
                    self.state.set_pc(jump_pc as u32);
                } else {
                    self.state.set_pc(self.state.get_pc() + 4);
                }
                Ok(())
            }
            Instruction::BGEU(bgeu) => {
                if self.state.get_register_value_signed(bgeu.rs1.into())
                    >= self.state.get_register_value_signed(bgeu.rs2.into())
                {
                    let pc = self.state.get_pc();
                    let jump_pc = (pc as i32) + bgeu.imm12 as i32;
                    self.state.set_pc(jump_pc as u32);
                } else {
                    self.state.set_pc(self.state.get_pc() + 4);
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use alloc::collections::BTreeMap;

    use test_case::test_case;

    use crate::{elf::Program, state::State, vm::Vm};

    // TODO: Unignore this test once instructions required are supported
    #[test]
    #[ignore]
    fn check() {
        let _ = env_logger::try_init();
        let elf = std::fs::read("src/test.elf").unwrap();
        let max_mem_size = 1024 * 1024 * 1024; // 1 GB
        let program = Program::load_elf(&elf, max_mem_size);
        assert!(program.is_ok());
        let program = program.unwrap();
        let state = State::new(program);
        let mut vm = Vm::new(state);
        let res = vm.step();
        assert!(res.is_ok());
    }

    // NOTE: For writing test cases please follow RISCV
    // calling convention for using registers in instructions.
    // Please check https://en.wikichip.org/wiki/risc-v/registers

    #[test_case(0x007302b3, 5, 6, 7, 60049, 50493; "add r5, r6, r7")]
    #[test_case(0x01FF8FB3, 31, 31, 31, 8981, 8981; "add r31, r31, r31")]
    fn add(word: u32, rd: usize, rs1: usize, rs2: usize, rs1_value: u32, rs2_value: u32) {
        let _ = env_logger::try_init();
        let mut image = BTreeMap::new();
        // at 0 address instruction add
        image.insert(0_u32, word);
        // set sys-call EXIT in x17(or a7)
        image.insert(4_u32, 0x05d00893_u32);
        // add ECALL to halt the program
        image.insert(8_u32, 0x00000073_u32);
        let program = Program {
            entry: 0_u32,
            image,
        };
        let mut state = State::new(program);
        state.set_register_value(rs1, rs1_value);
        state.set_register_value(rs2, rs2_value);
        let mut vm = Vm::new(state);
        let res = vm.step();
        assert!(res.is_ok());
        assert_eq!(vm.state.get_register_value(rd), rs1_value + rs2_value);
    }

    // Tests 2 cases:
    //   1) without overflow
    //   2) with overflow (0x12345678 << 0x08 (shift left by 3 bits) == 0x34567800)
    #[test_case(0x007312b3, 5, 6, 7, 7, 8; "sll r5, r6, r7")]
    #[test_case(0x013912b3, 5, 18, 19, 0x12345678, 0x08; "sll r5, r18, r19")]
    fn sll(word: u32, rd: usize, rs1: usize, rs2: usize, rs1_value: u32, rs2_value: u32) {
        let _ = env_logger::try_init();
        let mut image = BTreeMap::new();
        // at 0 address instruction sll
        image.insert(0_u32, word);
        // set sys-call EXIT in x17(or a7)
        image.insert(4_u32, 0x05d00893_u32);
        // add ECALL to halt the program
        image.insert(8_u32, 0x00000073_u32);
        let program = Program {
            entry: 0_u32,
            image,
        };
        let mut state = State::new(program);
        state.set_register_value(rs1, rs1_value);
        state.set_register_value(rs2, rs2_value);
        let mut vm = Vm::new(state);
        let res = vm.step();
        assert!(res.is_ok());
        assert_eq!(vm.state.get_register_value(rd), rs1_value << rs2_value);
    }

    // Tests 2 cases:
    //   1) without overflow
    //   2) with underflow (0x87654321 >> 0x08 (logical shift right by 3 bits) == 0x00876543)
    #[test_case(0x007352b3, 5, 6, 7, 7, 8; "srl r5, r6, r7")]
    #[test_case(0x013952b3, 5, 18, 19, 0x87654321, 0x08; "srl r5, r18, r19")]
    fn srl(word: u32, rd: usize, rs1: usize, rs2: usize, rs1_value: u32, rs2_value: u32) {
        let _ = env_logger::try_init();
        let mut image = BTreeMap::new();
        // at 0 address instruction srl
        image.insert(0_u32, word);
        // set sys-call EXIT in x17(or a7)
        image.insert(4_u32, 0x05d00893_u32);
        // add ECALL to halt the program
        image.insert(8_u32, 0x00000073_u32);
        let program = Program {
            entry: 0_u32,
            image,
        };
        let mut state = State::new(program);
        state.set_register_value(rs1, rs1_value);
        state.set_register_value(rs2, rs2_value);
        let mut vm = Vm::new(state);
        let res = vm.step();
        assert!(res.is_ok());
        assert_eq!(vm.state.get_register_value(rd), rs1_value >> rs2_value);
    }

    // 0x87654321 >> 0x08 (arithmetic shift right by 3 bits) == 0xff876543
    #[test_case(0x413952b3, 5, 18, 19, 0x87654321, 0x08; "sra r5, r18, r19")]
    fn sra(word: u32, rd: usize, rs1: usize, rs2: usize, rs1_value: u32, rs2_value: u32) {
        let _ = env_logger::try_init();
        let mut image = BTreeMap::new();
        // at 0 address instruction sra
        image.insert(0_u32, word);
        // set sys-call EXIT in x17(or a7)
        image.insert(4_u32, 0x05d00893_u32);
        // add ECALL to halt the program
        image.insert(8_u32, 0x00000073_u32);
        let program = Program {
            entry: 0_u32,
            image,
        };
        let mut state = State::new(program);
        state.set_register_value(rs1, rs1_value);
        state.set_register_value(rs2, rs2_value);
        let mut vm = Vm::new(state);
        let res = vm.step();
        assert!(res.is_ok());
        assert_eq!(
            vm.state.get_register_value(rd),
            (rs1_value as i32 >> rs2_value as i32) as u32
        );
    }

    // x6 = 0x12345678 x7 = 0x0000ffff, x5 = 0x00000000
    // x18 = 0x82345678 x19 = 0x8000ffff, x5 = 0x00000001
    #[test_case(0x007322b3, 5, 6, 7, 0x12345678, 0x0000ffff; "slt r5, r6, r7")]
    #[test_case(0x013922b3, 5, 18, 19, 0x82345678, 0x0000ffff; "slt r5, r18, r19")]
    fn slt(word: u32, rd: usize, rs1: usize, rs2: usize, rs1_value: u32, rs2_value: u32) {
        let _ = env_logger::try_init();
        let mut image = BTreeMap::new();
        // at 0 address instruction slt
        image.insert(0_u32, word);
        // set sys-call EXIT in x17(or a7)
        image.insert(4_u32, 0x05d00893_u32);
        // add ECALL to halt the program
        image.insert(8_u32, 0x00000073_u32);
        let program = Program {
            entry: 0_u32,
            image,
        };
        let mut state = State::new(program);
        state.set_register_value(rs1, rs1_value);
        state.set_register_value(rs2, rs2_value);
        let mut vm = Vm::new(state);
        let res = vm.step();
        assert!(res.is_ok());
        let rs1_value = rs1_value as i32;
        let rs2_value = rs2_value as i32;
        assert_eq!(
            vm.state.get_register_value(rd),
            (rs1_value < rs2_value) as u32
        );
    }

    // x6 = 0x12345678 x7 = 0x0000ffff, x5 = 0x00000000
    // x18 = 0x12345678 x19 = 0x8000ffff, x5 = 0x00000001
    #[test_case(0x007332b3, 5, 6, 7, 0x12345678, 0x0000ffff; "sltu r5, r6, r7")]
    #[test_case(0x013932b3, 5, 18, 19, 0x12345678, 0x8000ffff; "sltu r5, r18, r19")]
    fn sltu(word: u32, rd: usize, rs1: usize, rs2: usize, rs1_value: u32, rs2_value: u32) {
        let _ = env_logger::try_init();
        let mut image = BTreeMap::new();
        // at 0 address instruction sltu
        image.insert(0_u32, word);
        // set sys-call EXIT in x17(or a7)
        image.insert(4_u32, 0x05d00893_u32);
        // add ECALL to halt the program
        image.insert(8_u32, 0x00000073_u32);
        let program = Program {
            entry: 0_u32,
            image,
        };
        let mut state = State::new(program);
        state.set_register_value(rs1, rs1_value);
        state.set_register_value(rs2, rs2_value);
        let mut vm = Vm::new(state);
        let res = vm.step();
        assert!(res.is_ok());
        assert_eq!(
            vm.state.get_register_value(rd),
            (rs1_value < rs2_value) as u32
        );
    }

    #[test_case(0x05d00393, 7, 0, 0, 93; "addi r7, r0, 93")]
    fn addi(word: u32, rd: usize, rs1: usize, rs1_value: u32, imm12: i16) {
        let _ = env_logger::try_init();
        let mut image = BTreeMap::new();
        // at 0 address instruction add
        image.insert(0_u32, word);
        // set sys-call EXIT in x17(or a7)
        image.insert(4_u32, 0x05d00893_u32);
        // add ECALL to halt the program
        image.insert(8_u32, 0x00000073_u32);
        let program = Program {
            entry: 0_u32,
            image,
        };
        let mut state = State::new(program);
        state.set_register_value(rs1, rs1_value);
        let mut vm = Vm::new(state);
        let res = vm.step();
        assert!(res.is_ok());
        let mut expected_value = rs1_value;
        if imm12.is_negative() {
            expected_value -= imm12.unsigned_abs() as u32;
        } else {
            expected_value += imm12 as u32;
        }
        assert_eq!(vm.state.get_register_value(rd), expected_value);
    }

    #[test_case(0x06430283, 5, 6, 100, 0, 127; "lb r5, 100(r6)")]
    #[test_case(0x06430283, 5, 6, 100, 200, 127; "lb r5, -100(r6) offset_negative")]
    #[test_case(0x06430283, 5, 6, 100, 0, -128; "lb r5, 100(r6) value_negative")]
    #[test_case(0x06430283, 5, 6, 100, 200, -128; "lb r5, -100(r6) offset_negative_value_negative")]
    fn lb(word: u32, rd: usize, rs1: usize, offset: i16, rs1_value: u32, memory_value: i8) {
        let _ = env_logger::try_init();
        let mut image = BTreeMap::new();
        // at 0 address instruction add
        image.insert(0_u32, word);
        // set sys-call EXIT in x17(or a7)
        image.insert(4_u32, 0x05d00893_u32);
        // add ECALL to halt the program
        image.insert(8_u32, 0x00000073_u32);
        let mut address: u32 = rs1_value;
        if offset.is_negative() {
            let abs_offset = offset.unsigned_abs() as u32;
            assert!(abs_offset <= rs1_value);
            address -= offset.unsigned_abs() as u32;
        } else {
            address += offset as u32;
        }
        image.insert(address, memory_value as u32);
        let program = Program {
            entry: 0_u32,
            image,
        };
        let mut state = State::new(program);
        state.set_register_value(rs1, rs1_value);
        let mut vm = Vm::new(state);
        let res = vm.step();
        assert!(res.is_ok());
        let mut expected_value = memory_value as u32;
        if memory_value.is_negative() {
            // extend the sign
            expected_value |= 0xffffff00;
        }
        assert_eq!(vm.state.get_register_value(rd), expected_value);
    }

    // TODO: Add more tests for JAL/JALR
    #[test]
    fn jal_jalr() {
        let _ = env_logger::try_init();
        let mut image = BTreeMap::new();
        // at 0 address instruction jal to 256
        // JAL x1, 256
        image.insert(0_u32, 0x100000ef);
        // set sys-call EXIT in x17(or a7)
        image.insert(4_u32, 0x05d00893_u32);
        // add ECALL to halt the program
        image.insert(8_u32, 0x00000073_u32);
        // set R5 to 100 so that it can be verified
        // that indeed control passed to this location
        // ADDI x5, x0, 100
        image.insert(256_u32, 0x06400293);
        // at 260 go back to address after JAL
        // JALR x0, x1, 0
        image.insert(260_u32, 0x00008067);
        let program = Program {
            entry: 0_u32,
            image,
        };
        let state = State::new(program);
        let mut vm = Vm::new(state);
        let res = vm.step();
        assert!(res.is_ok());
        assert!(vm.state.has_halted());
        assert_eq!(vm.state.get_register_value(5_usize), 100_u32);
    }

    #[test]
    fn beq() {
        let _ = env_logger::try_init();
        let mut image = BTreeMap::new();
        // at 0 address instruction BEQ to 256
        // BEQ x0, x1, 256
        image.insert(0_u32, 0x10100063);
        // set sys-call EXIT in x17(or a7)
        image.insert(4_u32, 0x05d00893_u32);
        // add ECALL to halt the program
        image.insert(8_u32, 0x00000073_u32);
        // set R5 to 100 so that it can be verified
        // that indeed control passed to this location
        // ADDI x5, x0, 100
        image.insert(256_u32, 0x06400293);
        // at 260 go back to address after BEQ
        // JAL x0, -256
        image.insert(260_u32, 0xf01ff06f);
        let program = Program {
            entry: 0_u32,
            image,
        };
        let state = State::new(program);
        let mut vm = Vm::new(state);
        let res = vm.step();
        assert!(res.is_ok());
        assert!(vm.state.has_halted());
        assert_eq!(vm.state.get_register_value(5_usize), 100_u32);
    }

    #[test]
    fn bne() {
        let _ = env_logger::try_init();
        let mut image = BTreeMap::new();
        // at 0 address instruction BNE to 256
        // BNE x0, x1, 256
        image.insert(0_u32, 0x10101063);
        // set sys-call EXIT in x17(or a7)
        image.insert(4_u32, 0x05d00893_u32);
        // add ECALL to halt the program
        image.insert(8_u32, 0x00000073_u32);
        // set R5 to 100 so that it can be verified
        // that indeed control passed to this location
        // ADDI x5, x0, 100
        image.insert(256_u32, 0x06400293);
        // at 260 go back to address after BNE
        // JAL x0, -256
        image.insert(260_u32, 0xf01ff06f);
        let program = Program {
            entry: 0_u32,
            image,
        };
        let mut state = State::new(program);
        state.set_register_value(1_usize, 1_u32);
        let mut vm = Vm::new(state);
        let res = vm.step();
        assert!(res.is_ok());
        assert!(vm.state.has_halted());
        assert_eq!(vm.state.get_register_value(5_usize), 100_u32);
    }

    #[test]
    fn blt() {
        let _ = env_logger::try_init();
        let mut image = BTreeMap::new();
        // at 0 address instruction BLT to 256
        // BLT x1, x0, 256
        image.insert(0_u32, 0x1000c063);
        // set sys-call EXIT in x17(or a7)
        image.insert(4_u32, 0x05d00893_u32);
        // add ECALL to halt the program
        image.insert(8_u32, 0x00000073_u32);
        // set R5 to 100 so that it can be verified
        // that indeed control passed to this location
        // ADDI x5, x0, 100
        image.insert(256_u32, 0x06400293);
        // at 260 go back to address after BLT
        // JAL x0, -256
        image.insert(260_u32, 0xf01ff06f);
        let program = Program {
            entry: 0_u32,
            image,
        };
        let mut state = State::new(program);
        // set R1 = -1
        state.set_register_value(1_usize, 0xffffffff);
        let mut vm = Vm::new(state);
        let res = vm.step();
        assert!(res.is_ok());
        assert!(vm.state.has_halted());
        assert_eq!(vm.state.get_register_value(5_usize), 100_u32);
    }

    #[test]
    fn bltu() {
        let _ = env_logger::try_init();
        let mut image = BTreeMap::new();
        // at 0 address instruction BLTU to 256
        // BLTU x1, x2, 256
        image.insert(0_u32, 0x1020e063);
        // set sys-call EXIT in x17(or a7)
        image.insert(4_u32, 0x05d00893_u32);
        // add ECALL to halt the program
        image.insert(8_u32, 0x00000073_u32);
        // set R5 to 100 so that it can be verified
        // that indeed control passed to this location
        // ADDI x5, x0, 100
        image.insert(256_u32, 0x06400293);
        // at 260 go back to address after BLTU
        // JAL x0, -256
        image.insert(260_u32, 0xf01ff06f);
        let program = Program {
            entry: 0_u32,
            image,
        };
        let mut state = State::new(program);
        state.set_register_value(1_usize, 0xfffffffe);
        state.set_register_value(2_usize, 0xffffffff);
        let mut vm = Vm::new(state);
        let res = vm.step();
        assert!(res.is_ok());
        assert!(vm.state.has_halted());
        assert_eq!(vm.state.get_register_value(5_usize), 100_u32);
    }

    #[test]
    fn bge() {
        let _ = env_logger::try_init();
        let mut image = BTreeMap::new();
        // at 0 address instruction BGE to 256
        // BGE x0, x1, 256
        image.insert(0_u32, 0x10105063);
        // set sys-call EXIT in x17(or a7)
        image.insert(4_u32, 0x05d00893_u32);
        // add ECALL to halt the program
        image.insert(8_u32, 0x00000073_u32);
        // set R5 to 100 so that it can be verified
        // that indeed control passed to this location
        // ADDI x5, x0, 100
        image.insert(256_u32, 0x06400293);
        // at 260 go back to address after BGE
        // JAL x0, -256
        image.insert(260_u32, 0xf01ff06f);
        let program = Program {
            entry: 0_u32,
            image,
        };
        let mut state = State::new(program);
        // set R1 = -1
        state.set_register_value(1_usize, 0xffffffff);
        let mut vm = Vm::new(state);
        let res = vm.step();
        assert!(res.is_ok());
        assert!(vm.state.has_halted());
        assert_eq!(vm.state.get_register_value(5_usize), 100_u32);
    }

    #[test]
    fn bgeu() {
        let _ = env_logger::try_init();
        let mut image = BTreeMap::new();
        // at 0 address instruction BGEU to 256
        // BGEU x2, x1, 256
        image.insert(0_u32, 0x10117063);
        // set sys-call EXIT in x17(or a7)
        image.insert(4_u32, 0x05d00893_u32);
        // add ECALL to halt the program
        image.insert(8_u32, 0x00000073_u32);
        // set R5 to 100 so that it can be verified
        // that indeed control passed to this location
        // ADDI x5, x0, 100
        image.insert(256_u32, 0x06400293);
        // at 260 go back to address after BGEU
        // JAL x0, -256
        image.insert(260_u32, 0xf01ff06f);
        let program = Program {
            entry: 0_u32,
            image,
        };
        let mut state = State::new(program);
        state.set_register_value(1_usize, 0xfffffffe);
        state.set_register_value(2_usize, 0xffffffff);
        let mut vm = Vm::new(state);
        let res = vm.step();
        assert!(res.is_ok());
        assert!(vm.state.has_halted());
        assert_eq!(vm.state.get_register_value(5_usize), 100_u32);
    }
}

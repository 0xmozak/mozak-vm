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
            self.state.set_pc(pc + 4);
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
                Ok(())
            }
            Instruction::SUB(sub) => {
                let res = self.state.get_register_value(sub.rs1.into())
                    - self.state.get_register_value(sub.rs2.into());
                self.state.set_register_value(sub.rd.into(), res);
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
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use alloc::collections::BTreeMap;

    use test_case::test_case;

    use crate::{elf::Program, state::State, vm::Vm};
    #[test]
    fn check() {
        let _ = env_logger::try_init();
        let elf = std::fs::read("src/test.elf").unwrap();
        let max_mem_size = 1 * 1024 * 1024 * 1024; // 1 GB
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
        state.set_register_value(rs1.into(), rs1_value);
        state.set_register_value(rs2.into(), rs2_value);
        let mut vm = Vm::new(state);
        let res = vm.step();
        assert!(res.is_ok());
        assert_eq!(vm.state.get_register_value(rd), rs1_value + rs2_value);
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
        state.set_register_value(rs1.into(), rs1_value);
        let mut vm = Vm::new(state);
        let res = vm.step();
        assert!(res.is_ok());
        let mut expected_value = rs1_value;
        if imm12.is_negative() {
            expected_value -= imm12.abs() as u32;
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
            let abs_offset = offset.abs() as u32;
            assert!(abs_offset <= rs1_value);
            address -= offset.abs() as u32;
        } else {
            address += offset as u32;
        }
        image.insert(address, memory_value as u32);
        let program = Program {
            entry: 0_u32,
            image,
        };
        let mut state = State::new(program);
        state.set_register_value(rs1.into(), rs1_value);
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
}

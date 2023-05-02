use anyhow::{anyhow, Result};

use crate::elf::Program;

pub struct State {
    halted: bool,
    registers: [u32; 32],
    pc: u32,
    program: Program,
}

impl State {
    pub fn new(program: Program) -> Self {
        Self {
            halted: false,
            registers: [0_u32; 32],
            pc: program.entry,
            program,
        }
    }
    pub fn halt(&mut self) {
        self.halted = true;
    }

    pub fn has_halted(&self) -> bool {
        self.halted
    }

    pub fn set_register_value(&mut self, index: usize, value: u32) {
        assert!(index < 32);
        self.registers[index] = value;
    }

    pub fn get_register_value(&self, index: usize) -> u32 {
        self.registers[index]
    }

    pub fn set_pc(&mut self, value: u32) {
        self.pc = value;
    }

    pub fn get_pc(&self) -> u32 {
        self.pc
    }

    pub fn load_u32(&self, addr: u32) -> Result<u32> {
        let word = self
            .program
            .image
            .get(&addr)
            .ok_or(anyhow!("Address invalid for image"))?;
        Ok(*word)
    }

    pub fn load_u8(&self, addr: u32) -> Result<u8> {
        let word = self
            .program
            .image
            .get(&addr)
            .ok_or(anyhow!("Address invalid for image"))?;
        Ok((*word & 0x000000ff) as u8)
    }

    pub fn load_u16(&self, addr: u32) -> Result<u16> {
        let word = self
            .program
            .image
            .get(&addr)
            .ok_or(anyhow!("Address invalid for image"))?;
        Ok((*word & 0x0000ffff) as u16)
    }
}

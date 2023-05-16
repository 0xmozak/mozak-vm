use anyhow::{anyhow, ensure, Result};

use crate::elf::Program;

pub struct State {
    halted: bool,
    registers: [u32; 32],
    pc: u32,
    memory: Vec<u8>,
}

impl State {
    pub fn new(program: Program) -> Self {
        let mut memory = vec![0_u8; 256 * 1024 * 1024];
        for (addr, data) in program.image.iter() {
            let addr = *addr as usize;
            let bytes = data.to_le_bytes();
            memory[addr..(4 + addr)].copy_from_slice(&bytes[..4]);
        }
        Self {
            halted: false,
            registers: [0_u32; 32],
            pc: program.entry,
            memory,
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
        // R0 is always 0
        if index != 0 {
            self.registers[index] = value;
        }
    }

    pub fn get_register_value(&self, index: usize) -> u32 {
        self.registers[index]
    }

    pub fn get_register_value_signed(&self, index: usize) -> i32 {
        let word = self.registers[index];
        if word & 0x80000000 != 0 {
            // convert from 2's complement
            0 - (!(word - 1)) as i32
        } else {
            word as i32
        }
    }

    pub fn set_pc(&mut self, value: u32) {
        self.pc = value;
    }

    pub fn get_pc(&self) -> u32 {
        self.pc
    }

    pub fn load_u32(&self, addr: u32) -> Result<u32> {
        const WORD_SIZE: usize = 4;
        assert_eq!(addr % WORD_SIZE as u32, 0, "unaligned load");
        let mut bytes = [0_u8; WORD_SIZE];
        for (i, byte) in bytes.iter_mut().enumerate() {
            *byte = self.load_u8(addr + i as u32)?;
        }
        Ok(u32::from_le_bytes(bytes))
    }

    pub fn store_u32(&mut self, addr: u32, value: u32) -> Result<()> {
        const WORD_SIZE: usize = 4;
        assert_eq!(addr % WORD_SIZE as u32, 0, "unaligned load");
        let bytes = value.to_le_bytes();
        for (i, byte) in bytes.iter().enumerate() {
            self.store_u8(addr + i as u32, *byte)?;
        }
        Ok(())
    }

    pub fn load_u8(&self, addr: u32) -> Result<u8> {
        ensure!(
            self.memory.len() >= addr as usize,
            anyhow!("Address outof bound")
        );
        Ok(self.memory[addr as usize])
    }

    pub fn store_u8(&mut self, addr: u32, value: u8) -> Result<()> {
        ensure!(
            self.memory.len() >= addr as usize,
            anyhow!("Address outof bound")
        );
        self.memory[addr as usize] = value;
        Ok(())
    }

    pub fn load_u16(&self, addr: u32) -> Result<u16> {
        let mut bytes = [0_u8; 2];
        bytes[0] = self.load_u8(addr)?;
        bytes[1] = self.load_u8(addr + 1_u32)?;
        Ok(u16::from_le_bytes(bytes))
    }

    pub fn store_u16(&mut self, addr: u32, value: u16) -> Result<()> {
        let bytes = value.to_le_bytes();
        self.store_u8(addr, bytes[0])?;
        self.store_u8(addr + 1_u32, bytes[1])?;
        Ok(())
    }
}

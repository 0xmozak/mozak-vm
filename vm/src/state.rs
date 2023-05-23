use anyhow::Result;
use im::hashmap::HashMap;

use crate::elf::Program;

#[derive(Clone)]
pub struct State {
    halted: bool,
    registers: [u32; 32],
    pc: u32,
    memory: HashMap<usize, u8>,
}

impl State {
    #[must_use]
    pub fn new(program: Program) -> Self {
        let mut memory = HashMap::new();
        for (addr, data) in &program.image {
            let addr = *addr as usize;
            let bytes = data.to_le_bytes();
            for a in 0..4 {
                memory.insert(addr + a, bytes[a]);
            }
        }
        Self {
            halted: false,
            registers: [0_u32; 32],
            pc: program.entry,
            memory,
        }
    }

    #[must_use]
    pub fn halt(mut self) -> Self {
        self.halted = true;
        self
    }

    #[must_use]
    pub fn has_halted(&self) -> bool {
        self.halted
    }

    /// Load a byte from memory
    ///
    /// # Panics
    /// This function panics, if you try to load into an invalid register.
    #[must_use]
    pub fn set_register_value(mut self, index: usize, value: u32) -> Self {
        assert!(index < 32);
        // R0 is always 0
        if index != 0 {
            self.registers[index] = value;
        }
        self
    }

    #[must_use]
    pub fn get_register_value(&self, index: usize) -> u32 {
        self.registers[index]
    }

    #[must_use]
    pub fn get_register_value_signed(&self, index: usize) -> i32 {
        self.get_register_value(index) as i32
    }

    #[must_use]
    pub fn set_pc(mut self, value: u32) -> Self {
        self.pc = value;
        self
    }

    #[must_use]
    pub fn get_pc(&self) -> u32 {
        self.pc
    }

    #[must_use]
    pub fn bump_pc(mut self) -> Self {
        self.pc += 4;
        self
    }

    /// Load a word from memory
    ///
    /// # Errors
    /// This function returns an error, if you try to load from an invalid
    /// address.
    ///
    /// # Panics
    /// This function panics, if you try to from an unaligned address.
    #[must_use]
    pub fn load_u32(&self, addr: u32) -> Result<u32> {
        const WORD_SIZE: usize = 4;
        assert_eq!(addr % WORD_SIZE as u32, 0, "unaligned load");
        let mut bytes = [0_u8; WORD_SIZE];
        for (i, byte) in bytes.iter_mut().enumerate() {
            *byte = self.load_u8(addr + i as u32)?;
        }
        Ok(u32::from_le_bytes(bytes))
    }

    /// Store a word to memory
    ///
    /// # Errors
    /// This function returns an error, if you try to store to an invalid
    /// address.
    /// # Panics
    /// This function panics, if you try to store to an unaligned address.
    #[must_use]
    pub fn store_u32(mut self, addr: u32, value: u32) -> Result<Self> {
        const WORD_SIZE: usize = 4;
        assert_eq!(addr % WORD_SIZE as u32, 0, "unaligned store");
        let bytes = value.to_le_bytes();
        for (i, byte) in bytes.iter().enumerate() {
            self = self.store_u8(addr + i as u32, *byte)?;
        }
        Ok(self)
    }

    /// Load a byte from memory
    ///
    /// # Errors
    /// This function returns an error, if you try to load from an invalid
    /// address.
    #[must_use]
    pub fn load_u8(&self, addr: u32) -> Result<u8> {
        // ensure!(
        //     self.memory.len() >= addr as usize,
        //     anyhow!("Address out of bounds")
        // );
        Ok(*self.memory.get(&(addr as usize)).unwrap_or(&0))
    }

    /// Store a byte to memory
    ///
    /// # Errors
    /// This function returns an error, if you try to store to an invalid
    /// address.
    #[must_use]
    pub fn store_u8(mut self, addr: u32, value: u8) -> Result<Self> {
        // ensure!(
        //     self.memory.len() >= addr as usize,
        //     anyhow!("Address out of bounds")
        // );
        self.memory.insert(addr as usize, value);
        Ok(self)
    }

    /// Load a halfword from memory
    ///
    /// # Errors
    /// This function returns an error, if you try to load from an invalid
    /// address.
    #[must_use]
    pub fn load_u16(&self, addr: u32) -> Result<u16> {
        let mut bytes = [0_u8; 2];
        bytes[0] = self.load_u8(addr)?;
        bytes[1] = self.load_u8(addr + 1_u32)?;
        Ok(u16::from_le_bytes(bytes))
    }

    /// Store a halfword to memory
    ///
    /// # Errors
    /// This function returns an error, if you try to store to an invalid
    /// address.
    #[must_use]
    pub fn store_u16(mut self, addr: u32, value: u16) -> Result<Self> {
        let bytes = value.to_le_bytes();
        self = self.store_u8(addr, bytes[0])?;
        self = self.store_u8(addr + 1_u32, bytes[1])?;
        Ok(self)
    }
}

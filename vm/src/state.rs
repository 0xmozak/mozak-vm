use anyhow::Result;
use im::hashmap::HashMap;
use proptest::prelude::*;
use risc0_core::field::baby_bear::BabyBearElem;

use crate::elf::Program;

// Create new type to use everywhere with just renaming
type FieldElement = BabyBearElem;

#[derive(Copy, Clone, Debug, Default)]
pub struct Register {
    lo: FieldElement,
    hi: FieldElement,
}

impl From<u32> for Register {
    fn from(value: u32) -> Self {
        Register {
            lo: FieldElement::new(value & 0xFFFF),
            hi: FieldElement::new(value >> 16),
        }
    }
}

impl From<Register> for u32 {
    fn from(val: Register) -> Self {
        val.hi.as_u32() << 16 | val.lo.as_u32()
    }
}

proptest! {
    #[test]
    fn round_trip(x in any::<u32>()) {
        let y: Register = x.into();
        let z: u32 = y.into();
        assert_eq!(x, z);
    }
}

/// State of our VM
///
/// Note: In general clone is not necessarily what you want, but in our case we
/// carefully picked the type of `memory` to be clonable in about O(1)
/// regardless of size. That way we can keep cheaply keep snapshots even at
/// every step of evaluation.
#[derive(Clone, Debug, Default)]
pub struct State {
    halted: bool,
    registers: [Register; 32],
    pc: Register,
    memory: HashMap<usize, FieldElement>,
}

impl From<Program> for State {
    fn from(program: Program) -> Self {
        let memory: HashMap<usize, FieldElement> = program
            .image
            .into_iter()
            .flat_map(|(addr, data)| {
                data.to_le_bytes()
                    .into_iter()
                    .enumerate()
                    .map(move |(a, byte)| (addr as usize + a, FieldElement::from(u32::from(byte))))
            })
            .collect();
        Self {
            pc: Register::from(program.entry),
            memory,
            ..Default::default()
        }
    }
}

impl State {
    pub fn halt(&mut self) {
        self.halted = true;
    }

    #[must_use]
    pub fn has_halted(&self) -> bool {
        self.halted
    }

    /// Load a byte from memory
    ///
    /// # Panics
    /// This function panics, if you try to load into an invalid register.
    pub fn set_register_value(&mut self, index: usize, value: u32) {
        // R0 is always 0
        if index != 0 {
            self.registers[index] = Register::from(value);
        }
    }

    #[must_use]
    pub fn get_register_value(&self, index: usize) -> u32 {
        self.registers[index].into()
    }

    #[must_use]
    pub fn get_register_value_signed(&self, index: usize) -> i32 {
        self.get_register_value(index) as i32
    }

    pub fn set_pc(&mut self, value: u32) {
        self.pc = Register::from(value);
    }

    #[must_use]
    pub fn get_pc(&self) -> u32 {
        self.pc.into()
    }

    /// Load a word from memory
    ///
    /// # Errors
    /// This function returns an error, if you try to load from an invalid
    /// address.
    pub fn load_u32(&self, addr: u32) -> Result<u32> {
        const WORD_SIZE: usize = 4;
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
    pub fn store_u32(&mut self, addr: u32, value: u32) -> Result<()> {
        let bytes = value.to_le_bytes();
        for (i, byte) in bytes.iter().enumerate() {
            self.store_u8(addr + i as u32, *byte)?;
        }
        Ok(())
    }

    /// Load a byte from memory
    ///
    /// # Errors
    /// This function returns an error, if you try to load from an invalid
    /// address.
    pub fn load_u8(&self, addr: u32) -> Result<u8> {
        Ok(self
            .memory
            .get(&(addr as usize))
            .map_or(0, |bb| bb.as_u32() as u8))
    }

    /// Store a byte to memory
    ///
    /// # Errors
    /// This function returns an error, if you try to store to an invalid
    /// address.
    pub fn store_u8(&mut self, addr: u32, value: u8) -> Result<()> {
        self.memory
            .insert(addr as usize, FieldElement::new(u32::from(value)));
        Ok(())
    }

    /// Load a halfword from memory
    ///
    /// # Errors
    /// This function returns an error, if you try to load from an invalid
    /// address.
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
    pub fn store_u16(&mut self, addr: u32, value: u16) -> Result<()> {
        let bytes = value.to_le_bytes();
        self.store_u8(addr, bytes[0])?;
        self.store_u8(addr + 1_u32, bytes[1])?;
        Ok(())
    }
}

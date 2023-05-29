use anyhow::Result;
use im::hashmap::HashMap;
use plonky2::field::goldilocks_field::GoldilocksField;
use plonky2::field::types::{Field, PrimeField64};
use proptest::prelude::*;

use crate::elf::Program;

/// State of our VM
///
/// Note: In general clone is not necessarily what you want, but in our case we
/// carefully picked the type of `memory` to be clonable in about O(1)
/// regardless of size. That way we can keep cheaply keep snapshots even at
/// every step of evaluation.
#[derive(Clone, Debug, Default)]
pub struct State {
    halted: bool,
    registers: [GoldilocksField; 32],
    pc: GoldilocksField,
    memory: HashMap<usize, GoldilocksField>,
}

impl From<Program> for State {
    fn from(program: Program) -> Self {
        let memory: HashMap<usize, GoldilocksField> = program
            .image
            .into_iter()
            .flat_map(|(addr, data)| {
                data.to_le_bytes()
                    .into_iter()
                    .enumerate()
                    .map(move |(a, byte)| {
                        (addr as usize + a, GoldilocksField::from_canonical_u8(byte))
                    })
            })
            .collect();
        Self {
            pc: GoldilocksField::from_canonical_u32(program.entry),
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
            self.registers[index] = GoldilocksField::from_canonical_u32(value);
        }
    }

    #[must_use]
    pub fn get_register_value(&self, index: usize) -> u32 {
        self.registers[index].to_canonical_u64() as u32
    }

    #[must_use]
    pub fn get_register_value_signed(&self, index: usize) -> i32 {
        self.get_register_value(index) as i32
    }

    pub fn set_pc(&mut self, value: u32) {
        self.pc = GoldilocksField::from_canonical_u32(value);
    }

    #[must_use]
    pub fn get_pc(&self) -> u32 {
        self.pc.to_canonical_u64() as u32
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
            .map_or(0, GoldilocksField::to_canonical_u64)
            .try_into()?)
    }

    /// Store a byte to memory
    ///
    /// # Errors
    /// This function returns an error, if you try to store to an invalid
    /// address.
    pub fn store_u8(&mut self, addr: u32, value: u8) -> Result<()> {
        self.memory
            .insert(addr as usize, GoldilocksField::from_canonical_u8(value));
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

proptest! {
    #[test]
    fn round_trip_memory(addr in any::<u32>(), x in any::<u32>()) {
        let mut state: State = State::default();
        state.store_u32(addr, x).unwrap();
        let y = state.load_u32(addr).unwrap();
        assert_eq!(x, y);
    }
    #[test]
    fn round_trip_u32(x in any::<u32>()) {
        let field_el = GoldilocksField::from_canonical_u32(x);
        let y = field_el.to_canonical_u64();
        assert_eq!(u64::from(x), y);
    }
}

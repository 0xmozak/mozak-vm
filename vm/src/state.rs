use im::hashmap::HashMap;
use plonky2::field::goldilocks_field::GoldilocksField;
use plonky2::field::types::{Field, PrimeField64};
use proptest::prelude::*;

use crate::elf::Program;
use crate::instruction::{BTypeInst, ITypeInst, RTypeInst};

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

impl RTypeInst {
    pub fn register_op(&self, state: State, op: fn(u32, u32) -> u32) -> State {
        let rs1 = state.get_register_value(self.rs1.into());
        let rs2 = state.get_register_value(self.rs2.into());
        state
            .set_register_value(self.rd.into(), op(rs1, rs2))
            .bump_pc()
    }
}

impl ITypeInst {
    pub fn register_op(&self, state: State, op: fn(u32, u32) -> u32) -> State {
        let rs1 = state.get_register_value(self.rs1.into());
        state
            .set_register_value(self.rd.into(), op(rs1, self.imm as u32))
            .bump_pc()
    }

    pub fn memory_load(&self, state: State, op: fn(&[u8; 4]) -> u32) -> State {
        let addr: u32 = state
            .get_register_value(self.rs1.into())
            .wrapping_add(self.imm as u32);
        let mem = [
            state.load_u8(addr),
            state.load_u8(addr + 1),
            state.load_u8(addr + 2),
            state.load_u8(addr + 3),
        ];
        state.set_register_value(self.rd.into(), op(&mem)).bump_pc()
    }
}

impl BTypeInst {
    pub fn register_op(&self, state: State, op: fn(u32, u32) -> bool) -> State {
        let rs1 = state.get_register_value(self.rs1.into());
        let rs2 = state.get_register_value(self.rs2.into());
        if op(rs1, rs2) {
            state.bump_pc_n(self.imm as u32)
        } else {
            state.bump_pc()
        }
    }
}

impl State {
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
        // R0 is always 0
        if index != 0 {
            self.registers[index] = GoldilocksField::from_canonical_u32(value);
        }
        self
    }

    #[must_use]
    pub fn get_register_value(&self, index: usize) -> u32 {
        self.registers[index]
            .to_canonical_u64()
            .try_into()
            .expect("Could not convert u64 to u32")
    }

    #[must_use]
    pub fn set_pc(mut self, value: u32) -> Self {
        self.pc = GoldilocksField::from_canonical_u32(value);
        self
    }

    #[must_use]
    pub fn get_pc(&self) -> u32 {
        self.pc.to_canonical_u64() as u32
    }

    #[must_use]
    pub fn bump_pc(self) -> Self {
        self.bump_pc_n(4)
    }

    #[must_use]
    pub fn bump_pc_n(self, diff: u32) -> Self {
        let pc = self.get_pc();
        self.set_pc(pc.wrapping_add(diff))
    }

    /// Load a word from memory
    ///
    /// # Errors
    /// This function returns an error, if you try to load from an invalid
    /// address.
    #[must_use]
    pub fn load_u32(&self, addr: u32) -> u32 {
        const WORD_SIZE: usize = 4;
        let mut bytes = [0_u8; WORD_SIZE];
        for (i, byte) in bytes.iter_mut().enumerate() {
            *byte = self.load_u8(addr + i as u32);
        }
        u32::from_le_bytes(bytes)
    }

    /// Load a byte from memory
    ///
    /// # Errors
    /// This function returns an error, if you try to load from an invalid
    /// address.
    pub fn load_u8(&self, addr: u32) -> u8 {
        self.memory
            .get(&(addr as usize))
            .map_or(0, GoldilocksField::to_canonical_u64)
            .try_into()
            .unwrap()
    }

    /// Store a byte to memory
    ///
    /// # Errors
    /// This function returns an error, if you try to store to an invalid
    /// address.
    #[must_use]
    pub fn store_u8(mut self, addr: u32, value: u8) -> Self {
        self.memory
            .insert(addr as usize, GoldilocksField::from_canonical_u8(value));
        self
    }
}

proptest! {
    #[test]
    fn round_trip_memory(addr in any::<u32>(), x in any::<u32>()) {
        let mut state: State = State::default();
        state.store_u32(addr, x).unwrap();
        let y = state.load_u32(addr);
        assert_eq!(x, y);
    }
    #[test]
    fn round_trip_u32(x in any::<u32>()) {
        let field_el = GoldilocksField::from_canonical_u32(x);
        let y = field_el.to_canonical_u64();
        assert_eq!(u64::from(x), y);
    }
}

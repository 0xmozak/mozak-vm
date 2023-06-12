use im::hashmap::HashMap;
use log::trace;

use crate::elf::{Code, Program};
use crate::instruction::{Data, Instruction};

/// State of our VM
///
/// Note: In general clone is not necessarily what you want, but in our case we
/// carefully picked the type of `memory` to be clonable in about O(1)
/// regardless of size. That way we can keep cheaply keep snapshots even at
/// every step of evaluation.
#[derive(Clone, Debug, Default)]
pub struct State {
    pub clk: u64,
    halted: bool,
    registers: [u32; 32],
    pc: u32,
    memory: HashMap<u32, u8>,
    // NOTE: meant to be immutable.
    // TODO(Matthias): replace with an immutable reference,
    // but need to sort out life-times first
    // (ie sort out where the original lives.)
    // This ain't super-urgent, because im::hashmap::HashMap is O(1) to clone.
    code: Code,
}

impl From<Program> for State {
    fn from(program: Program) -> Self {
        let memory: HashMap<u32, u8> = program.image;
        let code = program.code;
        Self {
            pc: program.entry,
            code,
            memory,
            ..Default::default()
        }
    }
}

impl State {
    #[must_use]
    pub fn register_op<F>(self, data: &Data, op: F) -> Self
    where
        F: FnOnce(u32, u32, u32) -> u32,
    {
        let rs1 = self.get_register_value(data.rs1.into());
        let rs2 = self.get_register_value(data.rs2.into());
        let imm: u32 = data.imm;
        self.set_register_value(data.rd.into(), op(rs1, rs2, imm))
            .bump_pc()
    }

    #[must_use]
    pub fn memory_load(self, data: &Data, op: fn(&[u8; 4]) -> u32) -> Self {
        let addr: u32 = self
            .get_register_value(data.rs1.into())
            .wrapping_add(data.imm);
        let mem = [
            self.load_u8(addr),
            self.load_u8(addr + 1),
            self.load_u8(addr + 2),
            self.load_u8(addr + 3),
        ];
        self.set_register_value(data.rd.into(), op(&mem)).bump_pc()
    }

    // TODO(Matthias): this used to use a register_op.
    #[must_use]
    pub fn branch_op(self, data: &Data, op: fn(u32, u32) -> bool) -> State {
        let rs1 = self.get_register_value(data.rs1.into());
        let rs2 = self.get_register_value(data.rs2.into());
        if op(rs1, rs2) {
            self.bump_pc_n(data.imm)
        } else {
            self.bump_pc()
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
            self.registers[index] = value;
        }
        self
    }

    #[must_use]
    pub fn get_register_value(&self, index: usize) -> u32 {
        self.registers[index]
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
    pub fn bump_pc(self) -> Self {
        self.bump_pc_n(4)
    }

    #[must_use]
    pub fn bump_pc_n(self, diff: u32) -> Self {
        let pc = self.get_pc();
        self.set_pc(pc.wrapping_add(diff))
    }

    #[must_use]
    pub fn bump_clock(mut self) -> Self {
        self.clk += 1;
        self
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
    /// # Panics
    /// This function panics if the conversion from `u32` to a `u8` fails, which
    /// is an internal error.
    #[must_use]
    pub fn load_u8(&self, addr: u32) -> u8 {
        self.memory.get(&addr).copied().unwrap_or_default()
    }

    /// Store a byte to memory
    ///
    /// # Errors
    /// This function returns an error, if you try to store to an invalid
    /// address.
    #[must_use]
    pub fn store_u8(mut self, addr: u32, value: u8) -> Self {
        self.memory.insert(addr, value);
        self
    }

    #[must_use]
    pub fn current_instruction(&self) -> Instruction {
        let pc = self.get_pc();
        let inst = self.code.get_instruction(pc);
        trace!("PC: {pc:#x?}, Decoded Inst: {inst:?}");
        inst
    }
}

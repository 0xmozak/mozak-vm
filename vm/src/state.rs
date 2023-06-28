use im::hashmap::HashMap;
use log::trace;

use crate::elf::{Code, Data, Program};
use crate::instruction::{Args, Instruction};

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
        let Data(memory) = program.data;
        let code = program.code;
        Self {
            pc: program.entry,
            code,
            memory,
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Aux {
    // This could be an Option<u32>, but given how Risc-V instruction are specified,
    // 0 serves as a default value just fine.
    pub dst_val: u32,
    pub new_pc: u32,
    pub mem_addr: Option<u32>,
    pub will_halt: bool,
}

impl State {
    #[must_use]
    pub fn register_op<F>(self, data: &Args, op: F) -> (Aux, Self)
    where
        F: FnOnce(u32, u32, u32) -> u32, {
        let rs1 = self.get_register_value(data.rs1);
        let rs2 = self.get_register_value(data.rs2);
        let dst_val = op(rs1, rs2, data.imm);
        (
            Aux {
                dst_val,
                ..Aux::default()
            },
            self.set_register_value(data.rd, dst_val).bump_pc(),
        )
    }

    #[must_use]
    pub fn memory_load(self, data: &Args, op: fn(&[u8; 4]) -> u32) -> (Aux, Self) {
        let addr: u32 = self.get_register_value(data.rs1).wrapping_add(data.imm);
        let mem = [
            self.load_u8(addr),
            self.load_u8(addr + 1),
            self.load_u8(addr + 2),
            self.load_u8(addr + 3),
        ];
        let dst_val = op(&mem);
        (
            Aux {
                dst_val,
                mem_addr: Some(addr),
                ..Default::default()
            },
            self.set_register_value(data.rd, dst_val).bump_pc(),
        )
    }

    #[must_use]
    pub fn branch_op(self, data: &Args, op: fn(u32, u32) -> bool) -> (Aux, State) {
        let rs1 = self.get_register_value(data.rs1);
        let rs2 = self.get_register_value(data.rs2);
        (
            Aux::default(),
            if op(rs1, rs2) {
                self.set_pc(data.imm)
            } else {
                self.bump_pc()
            },
        )
    }
}

impl State {
    #[must_use]
    pub fn halt(mut self) -> Self {
        self.halted = true;
        self
    }

    #[must_use]
    pub fn has_halted(&self) -> bool { self.halted }

    /// Load a byte from memory
    ///
    /// # Panics
    /// This function panics, if you try to load into an invalid register.
    #[must_use]
    pub fn set_register_value(mut self, index: u8, value: u32) -> Self {
        // R0 is always 0
        if index != 0 {
            self.registers[usize::from(index)] = value;
        }
        self
    }

    #[must_use]
    pub fn get_register_value(&self, index: u8) -> u32 { self.registers[usize::from(index)] }

    #[must_use]
    pub fn set_pc(mut self, value: u32) -> Self {
        self.pc = value;
        self
    }

    #[must_use]
    pub fn get_pc(&self) -> u32 { self.pc }

    #[must_use]
    pub fn bump_pc(self) -> Self { self.bump_pc_n(4) }

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
        for (i, byte) in (0_u32..).zip(bytes.iter_mut()) {
            *byte = self.load_u8(addr + i);
        }
        u32::from_le_bytes(bytes)
    }

    /// Load a byte from memory
    #[must_use]
    pub fn load_u8(&self, addr: u32) -> u8 { self.memory.get(&addr).copied().unwrap_or_default() }

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

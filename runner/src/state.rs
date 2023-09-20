use std::rc::Rc;

use anyhow::{anyhow, Result};
use im::hashmap::HashMap;
use log::trace;

use crate::elf::{Code, Data, Program};
use crate::instruction::{Args, Instruction};
#[cfg(feature = "serialize")]
use serde::{Deserialize, Serialize};

/// State of our VM
///
/// Note: In general clone is not necessarily what you want, but in our case we
/// carefully picked the type of `memory` to be clonable in about O(1)
/// regardless of size. That way we can keep cheaply keep snapshots even at
/// every step of evaluation.
///
/// A note on memory structuring: The `State` follows a [modified Harvard architecture](https://en.wikipedia.org/wiki/Modified_Harvard_architecture).
/// As such we effectively have separate address spaces for code and memory.
/// 'Modified' means that we pre-populate the memory address space with a copy
/// of the code. Writing to that copy does not change the instructions.
///
/// You can think of this as instructions being cached at the start of the
/// program and that cache never updating afterwards.
///
/// This is very similar to what many real world CPUs, including Risc-V ones, do
/// by default. The FENCE instruction can be used to make the CPU update the
/// instruction cache on many CPUs.  But we deliberately don't support that
/// usecase.
#[derive(Clone, Debug, Default)]
pub struct State {
    pub clk: u64,
    pub halted: bool,
    pub registers: [u32; 32],
    pub pc: u32,
    pub rw_memory: HashMap<u32, u8>,
    pub ro_memory: HashMap<u32, u8>,
    pub io_tape: IoTape,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct IoTape(pub Rc<[u8]>);

impl std::ops::Deref for IoTape {
    type Target = [u8];

    fn deref(&self) -> &Self::Target { &self.0 }
}

impl Default for IoTape {
    fn default() -> Self { IoTape(vec![].into()) }
}

#[allow(clippy::similar_names)]
impl From<Program> for State {
    fn from(
        Program {
            ro_code: Code(_),
            rw_memory: Data(rw_memory),
            ro_memory: Data(ro_memory),
            entry_point: pc,
            io_tape,
        }: Program,
    ) -> Self {
        Self {
            pc,
            rw_memory,
            ro_memory,
            io_tape,
            ..Default::default()
        }
    }
}

impl From<&Program> for State {
    fn from(program: &Program) -> Self { Self::from(program.clone()) }
}

/// Auxiliary information about the instruction execution
#[derive(Debug, Clone, Default)]
pub struct Aux {
    // This could be an Option<u32>, but given how Risc-V instruction are specified,
    // 0 serves as a default value just fine.
    pub dst_val: u32,
    pub new_pc: u32,
    pub mem_addr: Option<u32>,
    pub will_halt: bool,
    pub op1: u32,
    pub op2: u32,
}

impl State {
    #[must_use]
    pub fn register_op<F>(self, data: &Args, op: F) -> (Aux, Self)
    where
        F: FnOnce(u32, u32) -> u32, {
        let op1 = self.get_register_value(data.rs1);
        let op2 = self.get_register_value(data.rs2).wrapping_add(data.imm);
        let dst_val = op(op1, op2);
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
        let addr: u32 = self.get_register_value(data.rs2).wrapping_add(data.imm);
        let mem = [
            self.load_u8(addr),
            self.load_u8(addr.wrapping_add(1)),
            self.load_u8(addr.wrapping_add(2)),
            self.load_u8(addr.wrapping_add(3)),
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
    #[allow(clippy::missing_panics_doc)]
    pub fn branch_op(self, data: &Args, op: fn(u32, u32) -> bool) -> (Aux, State) {
        let op1 = self.get_register_value(data.rs1);
        let op2 = self.get_register_value(data.rs2);
        (
            Aux::default(),
            if op(op1, op2) {
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
    ///
    /// For now, we decided that we will offer the program the full 4 GiB of
    /// address space you can get with 32 bits.
    /// So no u32 address is out of bounds.
    #[must_use]
    pub fn load_u8(&self, addr: u32) -> u8 {
        self.ro_memory
            .get(&addr)
            .or_else(|| self.rw_memory.get(&addr))
            .copied()
            .unwrap_or_default()
    }

    /// Store a byte to memory
    ///
    /// # Errors
    /// This function returns an error, if you try to store to an invalid
    /// address.
    pub fn store_u8(mut self, addr: u32, value: u8) -> Result<Self> {
        match self.ro_memory.entry(addr) {
            im::hashmap::Entry::Occupied(entry) => Err(anyhow!(
                "cannot write to ro_memory entry {:?}",
                (entry.key(), entry.get())
            )),
            im::hashmap::Entry::Vacant(_) => {
                self.rw_memory.insert(addr, value);
                Ok(self)
            }
        }
    }

    #[must_use]
    pub fn current_instruction(&self, program: &Program) -> Instruction {
        let pc = self.get_pc();
        let inst = program.ro_code.get_instruction(pc);
        trace!("PC: {pc:#x?}, Decoded Inst: {inst:?}");
        inst
    }
}

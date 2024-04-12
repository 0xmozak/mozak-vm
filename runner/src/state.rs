use std::iter::once;
use std::marker::PhantomData;
use std::rc::Rc;

use anyhow::{anyhow, Result};
use derive_more::{Deref, Display};
use im::hashmap::HashMap;
use im::HashSet;
use log::trace;
use plonky2::hash::hash_types::RichField;
use serde::{Deserialize, Serialize};

use crate::code::Code;
use crate::elf::{Data, Program, RuntimeArguments};
use crate::instruction::{Args, DecodingError, Instruction};
use crate::poseidon2::Poseidon2Entry;

pub fn read_bytes(buf: &[u8], index: &mut usize, num_bytes: usize) -> Vec<u8> {
    let remaining_len = buf.len() - *index;
    let limit = num_bytes.min(remaining_len);
    let read = buf[*index..(*index + limit)].to_vec();
    log::trace!(
        "read: 0x{:0x}, {:?}, data.len: {:?}, data: {:?}",
        index,
        remaining_len,
        buf.len(),
        read
    );

    *index += limit;
    read
}

/// State of RISC-V VM
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
/// This is very similar to what many real world CPUs, including RISC-V ones, do
/// by default. The FENCE instruction can be used to make the CPU update the
/// instruction cache on many CPUs.  But we deliberately don't support that
/// usecase.
#[derive(Clone, Debug)]
pub struct State<F: RichField> {
    /// Clock used to count how many execution are executed
    /// Also used to avoid infinite loop
    pub clk: u64,
    pub halted: bool,
    pub registers: [u32; 32],
    pub pc: u32,
    pub memory: StateMemory,
    pub private_tape: IoTape,
    pub public_tape: IoTape,
    pub call_tape: IoTape,
    pub event_tape: IoTape,
    _phantom: PhantomData<F>,
}

#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Clone, Default)]
pub struct StateMemory {
    pub data: HashMap<u32, u8>,
    pub is_read_only: HashSet<u32>,
}

impl StateMemory {
    fn new<I, J>(ro: I, rw: J) -> Self
    where
        I: Iterator<Item = HashMap<u32, u8>>,
        J: Iterator<Item = HashMap<u32, u8>>, {
        let ro: HashMap<u32, u8> = ro.flat_map(HashMap::into_iter).collect();
        let mut rw: HashMap<u32, u8> = rw.flat_map(HashMap::into_iter).collect();
        StateMemory {
            is_read_only: ro.keys().copied().collect(),
            data: {
                rw.extend(ro);
                rw
            },
        }
    }
}

#[derive(Clone, Debug, Deref, Serialize, Deserialize)]
pub struct IoTape {
    #[deref]
    pub data: Rc<[u8]>,
    pub read_index: usize,
}

impl Default for IoTape {
    fn default() -> Self {
        Self {
            data: [].into(),
            read_index: 0,
        }
    }
}

/// By default, all `State` start with `clk` 1. This is to differentiate
/// execution clocks (1 and above) from `clk` value of 0 which is
/// reserved for any initialisation concerns. e.g. memory initialization
/// prior to program execution, register initialization etc.
impl<F: RichField> Default for State<F> {
    fn default() -> Self {
        Self {
            clk: 2,
            halted: Default::default(),
            registers: Default::default(),
            pc: Default::default(),
            memory: StateMemory::default(),
            private_tape: IoTape::default(),
            public_tape: IoTape::default(),
            call_tape: IoTape::default(),
            event_tape: IoTape::default(),
            _phantom: PhantomData,
        }
    }
}

#[allow(clippy::similar_names)]
impl<F: RichField> From<Program> for State<F> {
    fn from(
        Program {
            ro_code: Code(_),
            rw_memory: Data(rw_memory),
            ro_memory: Data(ro_memory),
            entry_point: pc,
            mozak_ro_memory,
        }: Program,
    ) -> Self {
        Self {
            pc,
            memory: StateMemory::new(
                [
                    ro_memory,
                    mozak_ro_memory.map(HashMap::from).unwrap_or_default(),
                ]
                .into_iter(),
                [rw_memory].into_iter(),
            ),
            ..Default::default()
        }
    }
}

impl<F: RichField> From<&Program> for State<F> {
    fn from(program: &Program) -> Self { Self::from(program.clone()) }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct MemEntry {
    pub addr: u32,
    pub raw_value: u32,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Display, Default)]
#[repr(u8)]
pub enum IoOpcode {
    #[default]
    None,
    StorePrivate,
    StorePublic,
    StoreCallTape,
}

#[derive(Debug, Default, Clone)]
pub struct IoEntry {
    pub addr: u32,
    pub op: IoOpcode,
    pub data: Vec<u8>,
}

/// Auxiliary information about the instruction execution
#[derive(Debug, Clone, Default)]
pub struct Aux<F: RichField> {
    // This could be an Option<u32>, but given how RISC-V instruction are specified,
    // 0 serves as a default value just fine.
    pub dst_val: u32,
    pub new_pc: u32,
    pub mem: Option<MemEntry>,
    pub mem_addresses_used: Vec<u32>,
    pub will_halt: bool,
    pub op1: u32,
    pub op2: u32,
    pub op2_raw: u32,
    pub poseidon2: Option<Poseidon2Entry<F>>,
    pub io: Option<IoEntry>,
}

impl<F: RichField> State<F> {
    #[must_use]
    #[allow(clippy::similar_names)]
    // TODO(Roman): currently this function uses old io-tape mechanism (based on
    // `ecall`) once a new stark mechanics related to io-tapes will be added, this
    // function will be used only for old-io-tapes API, and another function
    // `new_mozak_elf` will be added specifically for new io-tapes mechanism
    // NOTE: currently, both mozak-elf and vanilla elf will use this API since there
    // is still no stark-backend that supports new-io-tapes
    pub fn legacy_ecall_api_new(
        Program {
            rw_memory: Data(rw_memory),
            ro_memory: Data(ro_memory),
            mozak_ro_memory,
            entry_point: pc,
            ..
        }: Program,
        RuntimeArguments { .. }: RuntimeArguments,
    ) -> Self {
        let memory = StateMemory::new(
            [
                ro_memory,
                mozak_ro_memory.map(HashMap::from).unwrap_or_default(),
            ]
            .into_iter(),
            once(rw_memory),
        );
        Self {
            pc,
            memory,
            ..Default::default()
        }
    }

    #[must_use]
    #[allow(clippy::similar_names)]
    /// # Panics
    /// should not panic since access to the `mozak_ro_memory.unwrap()` takes
    /// place after `is_some` check
    pub fn new(
        Program {
            rw_memory: Data(rw_memory),
            ro_memory: Data(ro_memory),
            entry_point: pc,
            mozak_ro_memory,
            ..
        }: Program,
    ) -> Self {
        Self {
            pc,
            memory: StateMemory::new(
                [
                    ro_memory,
                    mozak_ro_memory.map(HashMap::from).unwrap_or_default(),
                ]
                .into_iter(),
                once(rw_memory),
            ),
            ..Default::default()
        }
    }

    #[must_use]
    pub fn register_op<Fun>(self, data: &Args, op: Fun) -> (Aux<F>, Self)
    where
        Fun: FnOnce(u32, u32) -> u32, {
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
    /// # Panics
    ///
    /// Panics if conversion from `mem_addresses_used: Vec<u32>` into `mem: [u8;
    /// 4]` fails, though, this should typically not fail since we iterate only
    /// from (0..4).
    pub fn memory_load(
        self,
        data: &Args,
        bytes: u32,
        op: fn(&[u8; 4]) -> (u32, u32),
    ) -> (Aux<F>, Self) {
        let addr: u32 = self.get_register_value(data.rs2).wrapping_add(data.imm);
        let mut mem_addresses_used: Vec<u32> = (0..4).map(|i| addr.wrapping_add(i)).collect();

        let mem = mem_addresses_used
            .iter()
            .map(|&addr| self.load_u8(addr))
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();

        mem_addresses_used.truncate(bytes as usize);
        let (raw_value, dst_val) = op(&mem);

        (
            Aux {
                dst_val,
                mem: Some(MemEntry { addr, raw_value }),
                mem_addresses_used,
                ..Default::default()
            },
            self.set_register_value(data.rd, dst_val).bump_pc(),
        )
    }

    #[must_use]
    #[allow(clippy::missing_panics_doc)]
    pub fn branch_op(self, data: &Args, op: fn(u32, u32) -> bool) -> (Aux<F>, Self) {
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

impl<F: RichField> State<F> {
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
        self.memory.data.get(&addr).copied().unwrap_or_default()
    }

    /// Store a byte to memory
    ///
    /// # Errors
    /// This function returns an error, if you try to store to an invalid
    /// address.
    pub fn store_u8(mut self, addr: u32, value: u8) -> Result<Self> {
        if self.memory.is_read_only.contains(&addr) {
            Err(anyhow!(
                "cannot write to ro_memory: address - {:#0x}, value - {:#0x}",
                addr,
                value,
            ))
        } else {
            self.memory.data.insert(addr, value);
            Ok(self)
        }
    }

    #[must_use]
    pub fn current_instruction<'a>(
        &self,
        program: &'a Program,
    ) -> Option<&'a Result<Instruction, DecodingError>> {
        let pc = self.get_pc();
        let inst = program.ro_code.get_instruction(pc);
        let clk = self.clk;
        trace!("CLK: {clk:#?}, PC: {pc:#x?}, Decoded Inst: {inst:?}");
        inst
    }
}

use std::ops::Range;

use derive_more::{Deref, DerefMut, IntoIterator};
use elf::endian::LittleEndian;
use elf::segment::ProgramHeader;
use elf::string_table::StringTable;
use elf::symbol::SymbolTable;
use im::hashmap::HashMap;
use itertools::izip;
use serde::{Deserialize, Serialize};

/// Memory of RISC-V Program
///
/// A wrapper around a map from a 32-bit address to a byte of memory
#[derive(
    Clone, Debug, Default, Deref, Serialize, Deserialize, DerefMut, IntoIterator, PartialEq,
)]

pub struct Data(pub HashMap<u32, u8>);

/// A Mozak program runtime arguments, all fields are 4 LE bytes length prefixed
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RuntimeArguments {
    pub self_prog_id: Vec<u8>,
    pub cast_list: Vec<u8>,
    pub io_tape_private: Vec<u8>,
    pub io_tape_public: Vec<u8>,
    pub call_tape: Vec<u8>,
    pub event_tape: Vec<u8>,
}

impl RuntimeArguments {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.self_prog_id.is_empty()
            && self.cast_list.is_empty()
            && self.io_tape_private.is_empty()
            && self.io_tape_public.is_empty()
            && self.call_tape.is_empty()
            && self.event_tape.is_empty()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct MozakMemoryRegion {
    pub starting_address: u32,
    pub capacity: u32,
    pub data: Data,
}

impl From<&RuntimeArguments> for MozakMemory {
    fn from(args: &RuntimeArguments) -> Self {
        let mut mozak_ro_memory = MozakMemory::default();
        mozak_ro_memory
            .self_prog_id
            .fill(args.self_prog_id.as_slice());
        mozak_ro_memory.cast_list.fill(args.cast_list.as_slice());
        mozak_ro_memory
            .io_tape_public
            .fill(args.io_tape_public.as_slice());
        mozak_ro_memory
            .io_tape_private
            .fill(args.io_tape_private.as_slice());
        mozak_ro_memory.call_tape.fill(args.call_tape.as_slice());
        mozak_ro_memory.event_tape.fill(args.event_tape.as_slice());

        mozak_ro_memory
    }
}

impl MozakMemoryRegion {
    fn memory_range(&self) -> Range<u32> {
        self.starting_address..self.starting_address + self.capacity
    }

    pub fn fill(&mut self, data: &[u8]) {
        assert!(
            data.len() <= self.capacity.try_into().unwrap(),
            "data of length {:?} does not fit into address ({:x?}) with capacity {:?}",
            data.len(),
            self.starting_address,
            self.capacity,
        );
        for (index, &item) in izip!(self.starting_address.., data) {
            self.data.insert(index, item);
        }
        assert!(
            self.data.len() <= self.capacity.try_into().unwrap(),
            "data of length {:?} does not fit into address ({:x?}) with capacity {:?}",
            self.data.len(),
            self.starting_address,
            self.capacity,
        );
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MozakMemory {
    pub self_prog_id: MozakMemoryRegion,
    pub cast_list: MozakMemoryRegion,
    pub io_tape_private: MozakMemoryRegion,
    pub io_tape_public: MozakMemoryRegion,
    pub call_tape: MozakMemoryRegion,
    pub event_tape: MozakMemoryRegion,
}

impl From<MozakMemory> for HashMap<u32, u8> {
    fn from(mem: MozakMemory) -> Self {
        [
            mem.self_prog_id,
            mem.cast_list,
            mem.io_tape_private,
            mem.io_tape_public,
            mem.call_tape,
            mem.event_tape,
        ]
        .into_iter()
        .flat_map(|MozakMemoryRegion { data: Data(d), .. }| d.into_iter())
        .collect()
    }
}

impl Default for MozakMemory {
    fn default() -> Self {
        // These magic numbers taken from mozak-linker-script
        // TODO(Roman): Once `end-of-mozak-region` symbol will be added to linker-script
        // it will be possible to implement test that load mozak-empty-ELF and check
        // that all expected addresses and capacities are indeed aligned with the code.
        // We have test, that loads `empty-ELF` compiled with mozak-linker-script.
        // This test ensures that assumed symbols are defined.
        MozakMemory {
            self_prog_id: MozakMemoryRegion {
                starting_address: 0x2000_0000_u32,
                capacity: 0x20_u32,
                ..Default::default()
            },
            cast_list: MozakMemoryRegion {
                starting_address: 0x2000_0020_u32,
                capacity: 0x00FF_FFE0_u32,
                ..Default::default()
            },
            io_tape_public: MozakMemoryRegion {
                starting_address: 0x2100_0000_u32,
                capacity: 0x0F00_0000_u32,
                ..Default::default()
            },
            io_tape_private: MozakMemoryRegion {
                starting_address: 0x3000_0000_u32,
                capacity: 0x1000_0000_u32,
                ..Default::default()
            },
            call_tape: MozakMemoryRegion {
                starting_address: 0x4000_0000_u32,
                capacity: 0x0800_0000_u32,
                ..Default::default()
            },
            event_tape: MozakMemoryRegion {
                starting_address: 0x4800_0000_u32,
                capacity: 0x0800_0000_u32,
                ..Default::default()
            },
        }
    }
}

impl MozakMemory {
    // TODO(Roman): refactor this function, caller can parse p_vaddr, so pure u32
    // address will be enough
    pub(crate) fn is_mozak_ro_memory_address(&self, program_header: &ProgramHeader) -> bool {
        self.is_address_belongs_to_mozak_ro_memory(
            u32::try_from(program_header.p_vaddr)
                .expect("p_vaddr for zk-vm expected to be cast-able to u32"),
        )
    }

    #[must_use]
    pub fn is_address_belongs_to_mozak_ro_memory(&self, address: u32) -> bool {
        let mem_addresses = [
            self.self_prog_id.memory_range(),
            self.cast_list.memory_range(),
            self.io_tape_public.memory_range(),
            self.io_tape_private.memory_range(),
            self.call_tape.memory_range(),
            self.event_tape.memory_range(),
        ];
        log::trace!(
            "mozak-memory-addresses: {:?}, address: {:?}",
            mem_addresses,
            address
        );
        mem_addresses.iter().any(|r| r.contains(&address))
    }

    pub(crate) fn fill(
        &mut self,
        (symbol_table, string_table): &(SymbolTable<LittleEndian>, StringTable),
    ) {
        let symbol_map: HashMap<_, _> = symbol_table
            .iter()
            .map(|s| (string_table.get(s.st_name as usize).unwrap(), s.st_value))
            .collect();
        let get = |sym_name: &str| {
            u32::try_from(
                *symbol_map
                    .get(sym_name)
                    .unwrap_or_else(|| panic!("{sym_name} not found")),
            )
            .unwrap_or_else(|err| {
                panic!(
                    "{sym_name}'s address should be u32 cast-able:
        {err}"
                )
            })
        };

        self.self_prog_id.starting_address = get("_mozak_self_prog_id");
        self.cast_list.starting_address = get("_mozak_cast_list");
        self.io_tape_public.starting_address = get("_mozak_public_io_tape");
        self.io_tape_private.starting_address = get("_mozak_private_io_tape");
        self.call_tape.starting_address = get("_mozak_call_tape");
        self.event_tape.starting_address = get("_mozak_event_tape");
        // log::debug!("_mozak_call_tape: 0x{:0x}", self.call_tape.starting_address);

        // compute capacity, assume single memory region (refer to linker-script)
        self.self_prog_id.capacity = 0x20_u32;
        self.cast_list.capacity = 0x00FF_FFE0_u32;

        self.io_tape_public.capacity =
            self.io_tape_private.starting_address - self.io_tape_public.starting_address;
        // refer to linker-script to understand this magic number ...
        // TODO(Roman): to get rid off this magic number, we need to have `_end` symbol
        // in linker script This way we can compute capacity directly from
        // linker-script. Currently, test that loads empty ELF, compiled with
        // linker-script we not help us, since there is not symbol that defines
        // `end-of-mozak-region`...
        self.io_tape_private.capacity =
            self.call_tape.starting_address - self.io_tape_private.starting_address;
        self.call_tape.capacity =
            self.event_tape.starting_address - self.call_tape.starting_address;
        self.event_tape.capacity = 0x5000_0000 - self.event_tape.starting_address;
    }
}

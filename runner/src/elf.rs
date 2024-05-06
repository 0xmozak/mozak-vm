use std::cmp::{max, min};
use std::iter::repeat;
use std::ops::Range;

use anyhow::{anyhow, ensure, Result};
use derive_more::{Deref, DerefMut, IntoIterator};
use elf::endian::LittleEndian;
use elf::file::Class;
use elf::segment::{ProgramHeader, SegmentTable};
use elf::string_table::StringTable;
use elf::symbol::SymbolTable;
use elf::ElfBytes;
use im::hashmap::HashMap;
use itertools::{chain, iproduct, izip, Itertools};
use mozak_sdk::core::ecall::COMMITMENT_SIZE;
use serde::{Deserialize, Serialize};

use crate::code::Code;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct MozakMemoryRegion {
    pub starting_address: u32,
    pub capacity: u32,
    pub data: Data,
}

impl MozakMemoryRegion {
    fn memory_range(&self) -> Range<u32> {
        self.starting_address..self.starting_address + self.capacity
    }

    fn fill(&mut self, data: &[u8]) {
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
    pub private_tape: MozakMemoryRegion,
    pub public_tape: MozakMemoryRegion,
    pub call_tape: MozakMemoryRegion,
    pub event_tape: MozakMemoryRegion,
}

impl From<MozakMemory> for HashMap<u32, u8> {
    fn from(mem: MozakMemory) -> Self {
        [
            mem.self_prog_id,
            mem.cast_list,
            mem.private_tape,
            mem.public_tape,
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
            public_tape: MozakMemoryRegion {
                starting_address: 0x2100_0000_u32,
                capacity: 0x0F00_0000_u32,
                ..Default::default()
            },
            private_tape: MozakMemoryRegion {
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
    fn is_mozak_ro_memory_address(&self, program_header: &ProgramHeader) -> bool {
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
            self.public_tape.memory_range(),
            self.private_tape.memory_range(),
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

    fn fill(&mut self, (symbol_table, string_table): &(SymbolTable<LittleEndian>, StringTable)) {
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
        self.public_tape.starting_address = get("_mozak_public_io_tape");
        self.private_tape.starting_address = get("_mozak_private_io_tape");
        self.call_tape.starting_address = get("_mozak_call_tape");
        self.event_tape.starting_address = get("_mozak_event_tape");
        // log::debug!("_mozak_call_tape: 0x{:0x}", self.call_tape.starting_address);

        // compute capacity, assume single memory region (refer to linker-script)
        self.self_prog_id.capacity = 0x20_u32;
        self.cast_list.capacity = 0x00FF_FFE0_u32;

        self.public_tape.capacity =
            self.private_tape.starting_address - self.public_tape.starting_address;
        // refer to linker-script to understand this magic number ...
        // TODO(Roman): to get rid off this magic number, we need to have `_end` symbol
        // in linker script This way we can compute capacity directly from
        // linker-script. Currently, test that loads empty ELF, compiled with
        // linker-script we not help us, since there is not symbol that defines
        // `end-of-mozak-region`...
        self.private_tape.capacity =
            self.call_tape.starting_address - self.private_tape.starting_address;
        self.call_tape.capacity =
            self.event_tape.starting_address - self.call_tape.starting_address;
        self.event_tape.capacity = 0x5000_0000 - self.event_tape.starting_address;
    }
}

/// A Mozak program runtime arguments, all fields are 4 LE bytes length prefixed
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RuntimeArguments {
    pub self_prog_id: Vec<u8>,
    pub events_commitment_tape: [u8; COMMITMENT_SIZE],
    pub cast_list_commitment_tape: [u8; COMMITMENT_SIZE],
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

impl From<&RuntimeArguments> for MozakMemory {
    fn from(args: &RuntimeArguments) -> Self {
        let mut mozak_ro_memory = MozakMemory::default();
        mozak_ro_memory
            .self_prog_id
            .fill(args.self_prog_id.as_slice());
        mozak_ro_memory.cast_list.fill(args.cast_list.as_slice());
        mozak_ro_memory
            .public_tape
            .fill(args.io_tape_public.as_slice());
        mozak_ro_memory
            .private_tape
            .fill(args.io_tape_private.as_slice());
        mozak_ro_memory.call_tape.fill(args.call_tape.as_slice());
        mozak_ro_memory.event_tape.fill(args.event_tape.as_slice());
        mozak_ro_memory
    }
}

/// A RISC-V program
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Program {
    /// The entrypoint of the program
    pub entry_point: u32,

    /// Read-only section of memory
    /// `ro_memory` takes precedence, if a memory location is in both.
    pub ro_memory: Data,

    /// Read-write section of memory
    /// `ro_memory` takes precedence, if a memory location is in both.
    pub rw_memory: Data,

    /// Executable code of the ELF, read only
    pub ro_code: Code,

    /// Mozak run-time memory
    // Earlier our Program was completely determined by the ELF, and did not
    // differ from one run to the next.
    // Compare how the existing code doesn't add io_tape information to the Program, but to the
    // State. Conceptually, we are trying to replace this existing mechanism here, but currently we
    // decided to leave it as is, later on we may refactor it to be 3 structs (something like
    // this): Program, State, Init-Data. Currently during execution we have chain of states, and
    // each state has Aux-Data that has some debug-help info (like memory snapshot) of the whole
    // program. It is not really a perf problem since its actually a reference but, maybe later on
    // we will decide to refactor it, because this debug-help info wasn't really usefull much.
    pub mozak_ro_memory: Option<MozakMemory>,
}

/// Memory of RISC-V Program
///
/// A wrapper around a map from a 32-bit address to a byte of memory
#[derive(
    Clone, Debug, Default, Deref, Serialize, Deserialize, DerefMut, IntoIterator, PartialEq,
)]
pub struct Data(pub HashMap<u32, u8>);

impl From<HashMap<u32, u32>> for Program {
    fn from(image: HashMap<u32, u32>) -> Self {
        for (addr, val) in image.iter() {
            assert!(addr % 4 == 0, "Misaligned code: {addr:x} {val:x}");
        }
        let image: HashMap<u32, u8> = image
            .iter()
            .flat_map(move |(k, v)| (*k..).zip(v.to_le_bytes()))
            .collect::<HashMap<u32, u8>>();
        Self {
            entry_point: 0_u32,
            ro_code: Code::from(&image),
            ro_memory: Data::default(),
            rw_memory: Data(image),
            mozak_ro_memory: None,
        }
    }
}

impl From<HashMap<u32, u32>> for Data {
    fn from(image: HashMap<u32, u32>) -> Self {
        // Check for overlapping data
        //
        // For example, if someone specifies
        // 0: 0xDEAD_BEEF, 1: 0xDEAD_BEEF
        // we would have conflicting values for bytes 1, 2, and 3.
        if image.len() > 1 {
            for (i, ((key0, val0), (key1, val1))) in
                iproduct!(0..4, image.iter().sorted().circular_tuple_windows())
            {
                assert!(
                    key0.wrapping_add(i) != *key1,
                    "Overlapping data: {key0:x}:{val0:x} clashes with {key1:x}:{val1}"
                );
            }
        }
        Data(
            image
                .iter()
                .flat_map(move |(k, v)| {
                    (u64::from(*k)..)
                        .map(|k| u32::try_from(k).unwrap())
                        .zip(v.to_le_bytes())
                })
                .collect(),
        )
    }
}

impl Program {
    /// Vanilla load-elf - NOT expect "_mozak_*" symbols in link. Maybe we
    /// should rename it later, with `vanilla_` prefix
    ///
    /// # Errors
    /// Same as `Program::internal_load_elf`
    pub fn vanilla_load_elf(input: &[u8]) -> Result<Program> {
        let (_, entry_point, segments) = Program::parse_and_validate_elf(input)?;
        Ok(Program::internal_load_elf(
            input,
            entry_point,
            segments,
            |flags, _, _| {
                (flags & elf::abi::PF_R == elf::abi::PF_R)
                    && (flags & elf::abi::PF_W == elf::abi::PF_NONE)
            },
            None,
        ))
    }

    /// Mozak load-elf - expect "_mozak_*" symbols in link
    /// # Errors
    /// Same as `Program::internal_load_elf`
    /// # Panics
    /// Same as `Program::internal_load_elf`
    #[must_use]
    pub fn mozak_load_elf(
        input: &[u8],
        (elf, entry_point, segments): (ElfBytes<LittleEndian>, u32, SegmentTable<LittleEndian>),
    ) -> Program {
        // Information related to the `check_program_flags`
        // `&& (!mozak_memory.is_mozak_ro_memory_address(ph))` --> this line is used to
        // filter RO-addresses related to the mozak-ROM. Currently we don't
        // support filtering by sections and, we don't know if it even possible.
        // Mozak-ROM address are RO address and will be filled by loader-code
        // with arguments provided from outside. Mozak-ROM can be accessed as Read-ONLY
        // from rust code and currently no init code to this section is
        // supported.
        Program::internal_load_elf(
            input,
            entry_point,
            segments,
            |flags, ph, mozak_memory: &Option<MozakMemory>| {
                (flags & elf::abi::PF_R == elf::abi::PF_R)
                    && (flags & elf::abi::PF_W == elf::abi::PF_NONE)
                    && (!mozak_memory
                        .as_ref()
                        .expect("Expected to exist for mozak-elf")
                        .is_mozak_ro_memory_address(ph))
            },
            Some({
                let mut mm = MozakMemory::default();
                mm.fill(&elf.symbol_table().unwrap().unwrap());
                mm
            }),
        )
    }

    fn parse_and_validate_elf(
        input: &[u8],
    ) -> Result<(ElfBytes<LittleEndian>, u32, SegmentTable<LittleEndian>)> {
        let elf = ElfBytes::<LittleEndian>::minimal_parse(input)?;
        ensure!(elf.ehdr.class == Class::ELF32, "Not a 32-bit ELF");
        ensure!(
            elf.ehdr.e_machine == elf::abi::EM_RISCV,
            "Invalid machine type, must be RISC-V"
        );
        ensure!(
            elf.ehdr.e_type == elf::abi::ET_EXEC,
            "Invalid ELF type, must be executable"
        );
        let entry_point: u32 = elf.ehdr.e_entry.try_into()?;
        ensure!(entry_point % 4 == 0, "Misaligned entrypoint");
        let segments = elf
            .segments()
            .ok_or_else(|| anyhow!("Missing segment table"))?;
        ensure!(segments.len() <= 256, "Too many program headers");
        Ok((elf, entry_point, segments))
    }

    /// Initialize a RISC Program from a validated ELF file.
    #[allow(clippy::similar_names)]
    fn internal_load_elf(
        input: &[u8],
        entry_point: u32,
        segments: SegmentTable<LittleEndian>,
        check_program_flags: fn(
            flags: u32,
            program_headers: &ProgramHeader,
            mozak_memory: &Option<MozakMemory>,
        ) -> bool,
        mozak_ro_memory: Option<MozakMemory>,
    ) -> Program {
        let ro_memory = Data(Program::extract_elf_data(
            check_program_flags,
            input,
            &segments,
            &mozak_ro_memory,
        ));

        let rw_memory = Data(Program::extract_elf_data(
            |flags, _, _| flags == elf::abi::PF_R | elf::abi::PF_W,
            input,
            &segments,
            &mozak_ro_memory,
        ));

        // Because we are implementing a modified Harvard Architecture, we make an
        // independent copy of the executable segments. In practice,
        // instructions will be in a R_X segment, so their data will show up in ro_code
        // and ro_memory. (RWX segments would show up in ro_code and rw_memory.)
        let ro_code = Code::from(&Program::extract_elf_data(
            |flags, _, _| flags & elf::abi::PF_X == elf::abi::PF_X,
            input,
            &segments,
            &mozak_ro_memory,
        ));

        Program {
            entry_point,
            ro_memory,
            rw_memory,
            ro_code,
            mozak_ro_memory,
        }
    }

    fn extract_elf_data(
        check_program_flags: fn(
            flags: u32,
            program_headers: &ProgramHeader,
            mozak_memory: &Option<MozakMemory>,
        ) -> bool,
        input: &[u8],
        segments: &SegmentTable<LittleEndian>,
        mozak_memory: &Option<MozakMemory>,
    ) -> HashMap<u32, u8> {
        segments
            .iter()
            .filter(|program_header| {
                check_program_flags(program_header.p_flags, program_header, mozak_memory)
            })
            .map(|program_header| -> anyhow::Result<_> {
                let file_size: usize = program_header.p_filesz.try_into()?;
                let mem_size: usize = program_header.p_memsz.try_into()?;
                let vaddr: u32 = program_header.p_vaddr.try_into()?;
                let offset = program_header.p_offset.try_into()?;

                let min_size = min(file_size, mem_size);
                let max_size = max(file_size, mem_size);
                Ok((vaddr..).zip(
                    chain!(&input[offset..][..min_size], repeat(&0u8))
                        .take(max_size)
                        .copied(),
                ))
            })
            .flatten_ok()
            .try_collect()
            .expect("extract elf data should always succeed")
    }

    /// Loads a "mozak program" from static ELF and populates the reserved
    /// memory with runtime arguments
    ///
    /// # Errors
    /// Will return `Err` if the ELF file is invalid or if the entrypoint is
    /// invalid.
    ///
    /// # Panics
    /// When `Program::load_elf` or index as address is not cast-able to be u32
    /// cast-able
    pub fn mozak_load_program(elf_bytes: &[u8], args: &RuntimeArguments) -> Result<Program> {
        let mut program =
            Program::mozak_load_elf(elf_bytes, Program::parse_and_validate_elf(elf_bytes)?);
        let mozak_ro_memory = program
            .mozak_ro_memory
            .as_mut()
            .expect("MozakMemory should exist for mozak-elf case");
        // Context Variables address
        mozak_ro_memory
            .self_prog_id
            .fill(args.self_prog_id.as_slice());
        mozak_ro_memory.cast_list.fill(args.cast_list.as_slice());
        // IO public
        mozak_ro_memory
            .public_tape
            .fill(args.io_tape_public.as_slice());
        // IO private
        mozak_ro_memory
            .private_tape
            .fill(args.io_tape_private.as_slice());
        mozak_ro_memory.call_tape.fill(args.call_tape.as_slice());
        mozak_ro_memory.event_tape.fill(args.event_tape.as_slice());

        Ok(program)
    }

    /// Creates a [`Program`] with preinitialized mozak memory given its memory,
    /// [`Code`] and [`RuntimeArguments`].
    ///
    /// # Panics
    ///
    /// Panics if any of `ro_mem`, `rw_mem` or `ro_code` violates the memory
    /// space that [`MozakMemory`] takes.
    #[must_use]
    #[allow(clippy::similar_names)]
    pub fn create(
        ro_mem: &[(u32, u8)],
        rw_mem: &[(u32, u8)],
        ro_code: Code,
        args: &RuntimeArguments,
    ) -> Program {
        let ro_memory = Data(ro_mem.iter().copied().collect());
        let rw_memory = Data(rw_mem.iter().copied().collect());

        // Non-strict behavior is to allow successful creation when arguments parameter
        // is empty
        if args.is_empty() {
            return Program {
                ro_memory,
                rw_memory,
                ro_code,
                mozak_ro_memory: None,
                ..Default::default()
            };
        }

        let mozak_ro_memory = MozakMemory::from(args);
        let mem_iters = chain!(ro_mem.iter(), rw_mem.iter()).map(|(addr, _)| addr);
        let code_iter = ro_code.iter().map(|(addr, _)| addr);
        chain!(mem_iters, code_iter).for_each(|addr| {
            assert!(
                !mozak_ro_memory.is_address_belongs_to_mozak_ro_memory(*addr),
                "address: {addr} belongs to mozak-ro-memory - it is forbidden"
            );
        });
        Program {
            ro_memory,
            rw_memory,
            ro_code,
            mozak_ro_memory: Some(mozak_ro_memory),
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_serialize_deserialize() {
        let program = Program::default();
        let serialized = serde_json::to_string(&program).unwrap();
        let deserialized: Program = serde_json::from_str(&serialized).unwrap();
        assert_eq!(program, deserialized);
    }

    #[test]
    fn test_mozak_load_program_default() {
        Program::mozak_load_program(mozak_examples::EMPTY_ELF, &RuntimeArguments::default())
            .unwrap();
    }

    #[test]
    fn test_mozak_load_program() {
        let data = vec![0, 1, 2, 3];

        let mozak_ro_memory =
            Program::mozak_load_program(mozak_examples::EMPTY_ELF, &RuntimeArguments {
                self_prog_id: data.clone(),
                cast_list: data.clone(),
                io_tape_private: data.clone(),
                io_tape_public: data.clone(),
                event_tape: data.clone(),
                call_tape: data.clone(),
                ..Default::default()
            })
            .unwrap()
            .mozak_ro_memory
            .unwrap();

        assert_eq!(mozak_ro_memory.self_prog_id.data.len(), data.len());
        assert_eq!(mozak_ro_memory.cast_list.data.len(), data.len());
        assert_eq!(mozak_ro_memory.private_tape.data.len(), data.len());
        assert_eq!(mozak_ro_memory.public_tape.data.len(), data.len());
        assert_eq!(mozak_ro_memory.call_tape.data.len(), data.len());
        assert_eq!(mozak_ro_memory.event_tape.data.len(), data.len());
    }
}

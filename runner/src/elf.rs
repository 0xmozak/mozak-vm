use std::cmp::{max, min};
use std::collections::HashSet;
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
use serde::{Deserialize, Serialize};

use crate::decode::decode_instruction;
use crate::instruction::{DecodingError, Instruction};
use crate::util::load_u32;

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
    fn create() -> MozakMemory {
        MozakMemory {
            self_prog_id: MozakMemoryRegion::default(),
            cast_list: MozakMemoryRegion::default(),
            io_tape_private: MozakMemoryRegion::default(),
            io_tape_public: MozakMemoryRegion::default(),
            call_tape: MozakMemoryRegion::default(),
            event_tape: MozakMemoryRegion::default(),
        }
    }

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

/// A RISC-V program
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
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

/// Executable code of the ELF
///
/// A wrapper of a map from pc to [Instruction]
#[derive(Clone, Debug, Default, Deref, Serialize, Deserialize)]
pub struct Code(pub HashMap<u32, Result<Instruction, DecodingError>>);

/// Memory of RISC-V Program
///
/// A wrapper around a map from a 32-bit address to a byte of memory
#[derive(
    Clone, Debug, Default, Deref, Serialize, Deserialize, DerefMut, IntoIterator, PartialEq,
)]
pub struct Data(pub HashMap<u32, u8>);

impl Code {
    /// Get [Instruction] given `pc`
    #[must_use]
    pub fn get_instruction(&self, pc: u32) -> Option<&Result<Instruction, DecodingError>> {
        let Code(code) = self;
        code.get(&pc)
    }
}

impl From<&HashMap<u32, u8>> for Code {
    fn from(image: &HashMap<u32, u8>) -> Self {
        Self(
            image
                .keys()
                .map(|addr| addr & !3)
                .collect::<HashSet<_>>()
                .into_iter()
                .map(|key| (key, decode_instruction(key, load_u32(image, key))))
                .collect(),
        )
    }
}

// TODO: Right now, we only have convenient functions for initialising the
// rw_memory and ro_code. In the future we might want to add ones for ro_memory
// as well (or leave it to be manually constructed by the caller).
impl From<HashMap<u32, u8>> for Program {
    fn from(image: HashMap<u32, u8>) -> Self {
        Self {
            entry_point: 0_u32,
            ro_code: Code::from(&image),
            ro_memory: Data::default(),
            rw_memory: Data(image),
            mozak_ro_memory: None,
        }
    }
}

impl From<HashMap<u32, u32>> for Program {
    fn from(image: HashMap<u32, u32>) -> Self {
        for (addr, val) in image.iter() {
            assert!(addr % 4 == 0, "Misaligned code: {addr:x} {val:x}");
        }
        Self::from(
            image
                .iter()
                .flat_map(move |(k, v)| (*k..).zip(v.to_le_bytes()))
                .collect::<HashMap<u32, u8>>(),
        )
    }
}

impl From<HashMap<u32, u32>> for Data {
    #[allow(clippy::cast_possible_truncation)]
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
                .flat_map(move |(k, v)| (u64::from(*k)..).map(|k| k as u32).zip(v.to_le_bytes()))
                .collect(),
        )
    }
}

type CheckProgramFlags =
    fn(flags: u32, program_headers: &ProgramHeader, mozak_memory: &Option<MozakMemory>) -> bool;

impl Program {
    /// Vanilla load-elf - NOT expect "_mozak_*" symbols in link. Maybe we
    /// should rename it later, with `vanilla_` prefix
    /// # Errors
    /// Same as `Program::internal_load_elf`
    /// # Panics
    /// Same as `Program::internal_load_elf`
    /// TODO(Roman): Refactor this API to be aligned with `mozak_load_elf` -
    /// just return Program
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
                let mut mm = MozakMemory::create();
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

    /// Initialize a RISC Program from an appropriate ELF file
    ///
    /// # Errors
    /// Will return `Err` if the ELF file is invalid or if the entrypoint is
    /// invalid.
    ///
    /// # Panics
    // This function is actually mostly covered by tests, but it's too annoying to work out how to
    // tell tarpaulin that we haven't covered all the error conditions. TODO: write tests to
    // exercise the error handling?
    #[allow(clippy::similar_names)]
    fn internal_load_elf(
        input: &[u8],
        entry_point: u32,
        segments: SegmentTable<LittleEndian>,
        check_program_flags: fn(
            u32,
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
        check_program_flags: CheckProgramFlags,
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

    /// Loads a "risc-v program" from static ELF and populates the reserved
    /// memory with runtime arguments. Note: this function added mostly for
    /// convenience of the API. Later on, maybe we should rename it with prefix:
    /// `vanilla_`
    ///
    /// # Errors
    /// Will return `Err` if the ELF file is invalid or if the entrypoint is
    /// invalid.
    ///
    /// # Panics
    /// When `Program::load_elf` or index as address is not cast-able to u32
    /// cast-able
    pub fn vanilla_load_program(elf_bytes: &[u8]) -> Result<Program> {
        Program::vanilla_load_elf(elf_bytes)
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
            .io_tape_public
            .fill(args.io_tape_public.as_slice());
        // IO private
        mozak_ro_memory
            .io_tape_private
            .fill(args.io_tape_private.as_slice());
        mozak_ro_memory.call_tape.fill(args.call_tape.as_slice());
        mozak_ro_memory.event_tape.fill(args.event_tape.as_slice());

        Ok(program)
    }

    /// # Panics
    /// When some of the provided addresses (rw,ro,code) belongs to
    /// `mozak-ro-memory`
    /// # Errors
    /// When some of the provided addresses (rw,ro,code) belongs to
    /// `mozak-ro-memory`
    #[must_use]
    #[allow(clippy::similar_names)]
    pub fn create_with_args(
        ro_mem: &[(u32, u8)],
        rw_mem: &[(u32, u8)],
        ro_code: &Code,
        args: &RuntimeArguments,
    ) -> Program {
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
            ro_memory: Data(ro_mem.iter().copied().collect()),
            rw_memory: Data(rw_mem.iter().copied().collect()),
            ro_code: ro_code.clone(),
            mozak_ro_memory: Some(mozak_ro_memory),
            ..Default::default()
        }
    }

    /// # Panics
    /// When some of the provided addresses (rw,ro,code) belongs to
    /// `mozak-ro-memory` AND when arguments for mozak-ro-memory is not empty
    /// # Errors
    /// When some of the provided addresses (rw,ro,code) belongs to
    /// `mozak-ro-memory`
    /// Note: This function is mostly useful for risc-v native tests, and other
    /// tests that need the ability to run over full memory space, and don't
    /// use any mozak-ro-memory capabilities
    #[must_use]
    #[allow(clippy::similar_names)]
    #[cfg(any(feature = "test", test))]
    pub fn create(
        ro_mem: &[(u32, u8)],
        rw_mem: &[(u32, u8)],
        ro_code: &Code,
        args: &RuntimeArguments,
    ) -> Program {
        // Non-strict behavior is to allow successful creation when arguments parameter
        // is empty
        if args.is_empty() {
            return Program {
                ro_memory: Data(ro_mem.iter().copied().collect()),
                rw_memory: Data(rw_mem.iter().copied().collect()),
                ro_code: ro_code.clone(),
                mozak_ro_memory: None,
                ..Default::default()
            };
        }
        Program::create_with_args(ro_mem, rw_mem, ro_code, args)
    }
}

#[cfg(test)]
mod test {
    use crate::elf::{MozakMemory, MozakMemoryRegion, Program, RuntimeArguments};

    #[test]
    fn test_serialize_deserialize() {
        let program = Program::default();
        let serialized = serde_json::to_string(&program).unwrap();
        let deserialized: Program = serde_json::from_str(&serialized).unwrap();

        // Check that all object parameters are the same.
        assert_eq!(program.entry_point, deserialized.entry_point);
        assert_eq!(program.ro_memory.0, deserialized.ro_memory.0);
        assert_eq!(program.rw_memory.0, deserialized.rw_memory.0);
        assert_eq!(program.ro_code.0, deserialized.ro_code.0);
    }

    #[test]
    fn test_mozak_memory_region() {
        let mut mmr = MozakMemoryRegion {
            capacity: 10,
            ..Default::default()
        };
        mmr.fill(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
        assert_eq!(mmr.starting_address, 0);
        assert_eq!(mmr.capacity, 10);
        let data = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
        mmr.data.iter().for_each(|(k, v)| {
            assert_eq!(u8::try_from(*k).unwrap(), *v);
            assert_eq!(data[usize::try_from(*k).unwrap()], *v);
        });
    }

    #[test]
    fn test_empty_elf_with_empty_args() {
        let mozak_ro_memory =
            Program::mozak_load_program(mozak_examples::EMPTY_ELF, &RuntimeArguments::default())
                .unwrap()
                .mozak_ro_memory
                .unwrap();
        assert_eq!(mozak_ro_memory.io_tape_private.data.len(), 0);
        assert_eq!(mozak_ro_memory.io_tape_public.data.len(), 0);
        assert_eq!(mozak_ro_memory.call_tape.data.len(), 0);
    }

    #[test]
    fn test_empty_elf_with_args() {
        let mozak_ro_memory =
            Program::mozak_load_program(mozak_examples::EMPTY_ELF, &RuntimeArguments {
                self_prog_id: vec![0],
                cast_list: vec![0, 1],
                io_tape_private: vec![0, 1, 2],
                io_tape_public: vec![0, 1, 2, 3],
                call_tape: vec![0, 1, 2, 3, 4],
                event_tape: vec![0, 1, 2, 3, 4, 5],
            })
            .unwrap()
            .mozak_ro_memory
            .unwrap();
        assert_eq!(mozak_ro_memory.self_prog_id.data.len(), 1);
        assert_eq!(mozak_ro_memory.cast_list.data.len(), 2);
        assert_eq!(mozak_ro_memory.io_tape_private.data.len(), 3);
        assert_eq!(mozak_ro_memory.io_tape_public.data.len(), 4);
        assert_eq!(mozak_ro_memory.call_tape.data.len(), 5);
        assert_eq!(mozak_ro_memory.event_tape.data.len(), 6);
    }

    #[test]
    fn test_empty_elf_check_assumed_values() {
        // This test ensures mozak-loader & mozak-linker-script is indeed aligned
        let mozak_ro_memory =
            Program::mozak_load_program(mozak_examples::EMPTY_ELF, &RuntimeArguments::default())
                .unwrap()
                .mozak_ro_memory
                .unwrap();
        let test_mozak_ro_memory = MozakMemory::default();
        assert_eq!(mozak_ro_memory, test_mozak_ro_memory);
    }
}

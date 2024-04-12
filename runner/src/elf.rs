use std::cmp::{max, min};
use std::iter::repeat;

use anyhow::{anyhow, ensure, Result};
use elf::endian::LittleEndian;
use elf::file::Class;
use elf::segment::{ProgramHeader, SegmentTable};
use elf::ElfBytes;
use im::hashmap::HashMap;
use itertools::{chain, iproduct, Itertools};
use serde::{Deserialize, Serialize};

use crate::code::Code;
use crate::preinit_memory::{Data, MozakMemory, RuntimeArguments};

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
            })
            .unwrap()
            .mozak_ro_memory
            .unwrap();

        assert_eq!(mozak_ro_memory.self_prog_id.data.len(), data.len());
        assert_eq!(mozak_ro_memory.cast_list.data.len(), data.len());
        assert_eq!(mozak_ro_memory.io_tape_private.data.len(), data.len());
        assert_eq!(mozak_ro_memory.io_tape_public.data.len(), data.len());
        assert_eq!(mozak_ro_memory.call_tape.data.len(), data.len());
        assert_eq!(mozak_ro_memory.event_tape.data.len(), data.len());
    }
}

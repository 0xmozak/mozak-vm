use std::cmp::{max, min};
use std::iter::repeat;

use anyhow::{anyhow, ensure, Result};
use derive_more::{Deref, DerefMut, IntoIterator};
use elf::endian::LittleEndian;
use elf::file::Class;
use elf::segment::{ProgramHeader, SegmentTable};
use elf::ElfBytes;
use im::hashmap::HashMap;
use itertools::{chain, iproduct, Itertools};
use serde::{Deserialize, Serialize};

use crate::code::Code;

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
            |flags, _| {
                (flags & elf::abi::PF_R == elf::abi::PF_R)
                    && (flags & elf::abi::PF_W == elf::abi::PF_NONE)
            },
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
        (_elf, entry_point, segments): (ElfBytes<LittleEndian>, u32, SegmentTable<LittleEndian>),
    ) -> Program {
        // Information related to the `check_program_flags`
        // `&& (!mozak_memory.is_mozak_ro_memory_address(ph))` --> this line is used to
        // filter RO-addresses related to the mozak-ROM. Currently we don't
        // support filtering by sections and, we don't know if it even possible.
        // Mozak-ROM address are RO address and will be filled by loader-code
        // with arguments provided from outside. Mozak-ROM can be accessed as Read-ONLY
        // from rust code and currently no init code to this section is
        // supported.
        Program::internal_load_elf(input, entry_point, segments, |flags, _| {
            (flags & elf::abi::PF_R == elf::abi::PF_R)
                && (flags & elf::abi::PF_W == elf::abi::PF_NONE)
        })
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
        check_program_flags: fn(flags: u32, program_headers: &ProgramHeader) -> bool,
    ) -> Program {
        let ro_memory = Data(Program::extract_elf_data(
            check_program_flags,
            input,
            &segments,
        ));

        let rw_memory = Data(Program::extract_elf_data(
            |flags, _| flags == elf::abi::PF_R | elf::abi::PF_W,
            input,
            &segments,
        ));

        // Because we are implementing a modified Harvard Architecture, we make an
        // independent copy of the executable segments. In practice,
        // instructions will be in a R_X segment, so their data will show up in ro_code
        // and ro_memory. (RWX segments would show up in ro_code and rw_memory.)
        let ro_code = Code::from(&Program::extract_elf_data(
            |flags, _| flags & elf::abi::PF_X == elf::abi::PF_X,
            input,
            &segments,
        ));

        Program {
            entry_point,
            ro_memory,
            rw_memory,
            ro_code,
        }
    }

    fn extract_elf_data(
        check_program_flags: fn(flags: u32, program_headers: &ProgramHeader) -> bool,
        input: &[u8],
        segments: &SegmentTable<LittleEndian>,
    ) -> HashMap<u32, u8> {
        segments
            .iter()
            .filter(|program_header| check_program_flags(program_header.p_flags, program_header))
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

    /// Loads a [`Program`] from static ELF.
    ///
    /// # Errors
    /// Will return `Err` if the ELF file is invalid or if the entrypoint is
    /// invalid.
    ///
    /// # Panics
    /// When `Program::load_elf` or index as address is not cast-able to be u32
    /// cast-able
    pub fn mozak_load_program(elf_bytes: &[u8]) -> Result<Program> {
        let program =
            Program::mozak_load_elf(elf_bytes, Program::parse_and_validate_elf(elf_bytes)?);
        Ok(program)
    }

    /// Creates a [`Program`] with [`Code`].
    #[must_use]
    #[allow(clippy::similar_names)]
    pub fn create(ro_mem: &[(u32, u8)], rw_mem: &[(u32, u8)], ro_code: Code) -> Program {
        Program {
            ro_memory: Data(ro_mem.iter().copied().collect()),
            rw_memory: Data(rw_mem.iter().copied().collect()),
            ro_code,
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
        Program::mozak_load_program(mozak_examples::EMPTY_ELF).unwrap();
    }
}

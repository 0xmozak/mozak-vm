use std::collections::HashSet;

use anyhow::{anyhow, ensure, Result};
use derive_more::Deref;
use elf::endian::LittleEndian;
use elf::file::Class;
use elf::ElfBytes;
use im::hashmap::HashMap;
use itertools::{iproduct, Itertools};
use serde::{Deserialize, Serialize};

use crate::decode::decode_instruction;
use crate::instruction::Instruction;
use crate::util::load_u32;

/// A RISC-V program
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Program {
    /// The entrypoint of the program
    pub entry_point: u32,

    /// Read-only section of memory
    /// 'ro_memory' takes precedence, if a memory location is in both.
    pub ro_memory: Data,

    /// Read-write section of memory
    /// 'ro_memory' takes precedence, if a memory location is in both.
    pub rw_memory: Data,

    /// Executable code of the ELF, read only
    pub ro_code: Code,
}

/// Executable code of the ELF
///
/// A wrapper of a map from pc to [Instruction]
#[derive(Clone, Debug, Default, Deref, Serialize, Deserialize)]
pub struct Code(pub HashMap<u32, Instruction>);

/// Memory of RISC-V Program
///
/// A wrapper around a map from a 32-bit address to a byte of memory
#[derive(Clone, Debug, Default, Deref, Serialize, Deserialize)]
pub struct Data(pub HashMap<u32, u8>);

impl Code {
    /// Get [Instruction] given `pc`
    #[must_use]
    pub fn get_instruction(&self, pc: u32) -> Instruction {
        let Code(code) = self;
        code.get(&pc).copied().unwrap_or_default()
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

// TODO: Right now, we only have conventient functions for initialising the
// rw_memory and ro_code. In the future we might want to add ones for ro_memory
// as well (or leave it to be manually constructed by the caller).
impl From<HashMap<u32, u8>> for Program {
    fn from(image: HashMap<u32, u8>) -> Self {
        Self {
            entry_point: 0_u32,
            ro_code: Code::from(&image),
            ro_memory: Data::default(),
            rw_memory: Data(image),
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

impl Program {
    /// Initialize a RISC Program from an appropriate ELF file
    ///
    /// # Errors
    /// Will return `Err` if the ELF file is invalid or if the entrypoint is
    /// invalid.
    // This function is actually mostly covered by tests, but it's too annoying to work out how to
    // tell tarpaulin that we haven't covered all the error conditions. TODO: write tests to
    // exercise the error handling?
    #[allow(clippy::similar_names)]
    pub fn load_elf(input: &[u8]) -> Result<Program> {
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
            .section_headers()
            .ok_or_else(|| anyhow!("Missing segment table"))?;
        ensure!(segments.len() <= 256, "Too many program headers");

        let extract = |check_flags: fn(u32) -> bool| {
            segments
                .iter()
                // It is OK to cast this as u32 because we already check that we're reading a
                // 32-bit ELF. The elf parsing crate simply does an `as u64`
                // after parsing `sh_flags` as a u32: https://docs.rs/elf/latest/src/elf/section.rs.html#82
                .filter_map(|s: elf::section::SectionHeader| {
                    check_flags(u32::try_from(s.sh_flags).ok()?).then_some(s)
                })
                .map(|segment| -> Result<_> {
                    let file_size: usize = segment.sh_size.try_into()?;
                    let vaddr: u32 = segment.sh_addr.try_into()?;
                    let offset = segment.sh_offset.try_into()?;
                    Ok((vaddr..).zip(input[offset..].iter().take(file_size).copied()))
                })
                .flatten_ok()
                .try_collect()
        };

        let ro_memory = Data(extract(|flags| {
            flags & elf::abi::SHF_WRITE == elf::abi::SHF_NONE
        })?);
        let rw_memory = Data(extract(|flags| {
            (flags & elf::abi::SHF_ALLOC == elf::abi::SHF_ALLOC)
                && (flags & elf::abi::SHF_WRITE == elf::abi::SHF_WRITE)
        })?);
        // Because we are implementing a modified Harvard Architecture, we make an
        // independent copy of the executable segments. In practice,
        // instructions will be in a R_X segment, so their data will show up in ro_code
        // and ro_memory. (RWX segments would show up in ro_code and rw_memory.)
        let ro_code = Code::from(&extract(|flags| {
            flags & elf::abi::SHF_EXECINSTR == elf::abi::SHF_EXECINSTR
        })?);

        Ok(Program {
            entry_point,
            ro_memory,
            rw_memory,
            ro_code,
        })
    }
}

#[cfg(test)]
mod test {
    use crate::elf::Program;

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
}

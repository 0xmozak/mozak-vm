// Copyright 2023 MOZAK.

use std::collections::HashSet;

use anyhow::{anyhow, ensure, Result};
use derive_more::Deref;
use elf::endian::LittleEndian;
use elf::file::Class;
use elf::segment::ProgramHeader;
use elf::ElfBytes;
use im::hashmap::HashMap;
use itertools::Itertools;

use crate::decode::decode_instruction;
use crate::instruction::Instruction;
use crate::util::load_u32;

/// A RISC program
#[derive(Debug, Default)]
pub struct Program {
    /// The entrypoint of the program
    pub entry_point: u32,

    /// The initial memory image
    pub data: Data,
    /// Executable code
    pub code: Code,
}

#[derive(Clone, Debug, Default, Deref)]
pub struct Code(pub HashMap<u32, Instruction>);

#[derive(Clone, Debug, Default, Deref)]
pub struct Data(pub HashMap<u32, u8>);

impl Code {
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

impl From<HashMap<u32, u8>> for Program {
    #[tarpaulin::skip]
    fn from(image: HashMap<u32, u8>) -> Self {
        Self {
            entry_point: 0_u32,
            code: Code::from(&image),
            data: Data(image),
        }
    }
}

impl From<HashMap<u32, u32>> for Data {
    #[allow(clippy::cast_possible_truncation)]
    fn from(image: HashMap<u32, u32>) -> Self {
        Data(
            image
                .iter()
                .flat_map(move |(k, v)| (u64::from(*k)..).map(|k| k as u32).zip(v.to_le_bytes()))
                .collect(),
        )
    }
}

impl From<HashMap<u32, u32>> for Program {
    fn from(image: HashMap<u32, u32>) -> Self {
        let image = image
            .iter()
            .flat_map(move |(k, v)| (*k..).zip(v.to_le_bytes()))
            .collect();
        Self {
            entry_point: 0_u32,
            code: Code::from(&image),
            data: Data(image),
        }
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
    #[tarpaulin::skip]
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
            .segments()
            .ok_or_else(|| anyhow!("Missing segment table"))?;
        ensure!(segments.len() <= 256, "Too many program headers");

        let extract = |required_flags| {
            segments
                .iter()
                .filter(|s: &ProgramHeader| s.p_type == elf::abi::PT_LOAD)
                .filter(|s| s.p_flags & required_flags == required_flags)
                .map(|segment| -> Result<_> {
                    let file_size: usize = segment.p_filesz.try_into()?;
                    let mem_size: usize = segment.p_memsz.try_into()?;
                    let vaddr: u32 = segment.p_vaddr.try_into()?;
                    let offset = segment.p_offset.try_into()?;
                    Ok((vaddr..).zip(
                        input[offset..offset + std::cmp::min(file_size, mem_size)]
                            .iter()
                            .copied(),
                    ))
                })
                .flatten_ok()
                .try_collect()
        };

        let data = Data(extract(elf::abi::PF_NONE)?);
        let code = extract(elf::abi::PF_X)?;
        let code = Code::from(&code);
        Ok(Program {
            entry_point,
            data,
            code,
        })
    }
}

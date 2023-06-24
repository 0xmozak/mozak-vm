// Copyright 2023 MOZAK.

use std::collections::HashSet;

use anyhow::{anyhow, ensure, Error, Result};
use derive_more::Deref;
use elf_rs::{ElfClass, ElfEndian, ElfFile, ElfMachine, ElfType, ProgramHeaderFlags, ProgramType};
use im::hashmap::HashMap;
use itertools::Itertools;

use crate::decode::decode_instruction;
use crate::instruction::Instruction;
use crate::util::load_u32;

/// A RISC program
#[derive(Debug, Default)]
pub struct Program {
    /// The entrypoint of the program
    pub entry: u32,

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
            entry: 0_u32,
            code: Code::from(&image),
            data: Data(image),
        }
    }
}

impl From<HashMap<u32, u32>> for Data {
    fn from(image: HashMap<u32, u32>) -> Self {
        Data(
            image
                .iter()
                .flat_map(move |(k, v)| (*k..).zip(v.to_le_bytes().into_iter()))
                .collect(),
        )
    }
}

impl From<HashMap<u32, u32>> for Program {
    fn from(image: HashMap<u32, u32>) -> Self {
        let image = image
            .iter()
            .flat_map(move |(k, v)| (*k..).zip(v.to_le_bytes().into_iter()))
            .collect();
        Self {
            entry: 0_u32,
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
        let elf = elf_rs::Elf::from_bytes(input).map_err(|e| anyhow!("Invalid ELF: {e:?}"))?;

        let h = elf.elf_header();
        ensure!(
            h.endianness() == ElfEndian::LittleEndian,
            "Not little-endian ELF"
        );
        ensure!(h.class() == ElfClass::Elf32, "Not a 32-bit ELF");
        ensure!(
            h.machine() == ElfMachine::RISC_V,
            "Invalid machine type, must be RISC-V"
        );
        ensure!(
            h.elftype() == ElfType::ET_EXEC,
            "Invalid ELF type, must be executable"
        );
        let entry = h.entry_point().try_into()?;
        ensure!(entry % 4 != 0, "Misaligned entrypoint");
        let extract = |flags| {
            elf.program_header_iter()
                .filter(|h| h.ph_type() == ProgramType::LOAD)
                .filter(|h| h.flags().contains(flags))
                .map(|h| {
                    let content = h.content().ok_or(anyhow!(""))?;
                    let v: u32 = h.vaddr().try_into()?;
                    Ok((v..).zip(content.iter().copied()))
                })
                .flatten_ok()
                .try_collect::<(u32, u8), im::HashMap<u32, u8>, Error>()
        };
        let data = Data(extract(ProgramHeaderFlags::empty())?);
        let code = Code::from(&extract(ProgramHeaderFlags::EXECUTE)?);
        Ok(Program { entry, data, code })
    }
}

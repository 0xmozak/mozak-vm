// Copyright 2023 MOZAK.

use alloc::collections::BTreeMap;

use anyhow::{anyhow, bail, Result};
use elf::{endian::LittleEndian, file::Class, ElfBytes};

/// A RISC program
pub struct Program {
    /// The entrypoint of the program
    pub entry: u32,

    /// The initial memory image
    pub image: BTreeMap<u32, u32>,
}

impl Program {
    /// Initialize a RISC Program from an appropriate ELF file
    pub fn load_elf(input: &[u8], max_mem: u32) -> Result<Program> {
        let mut image: BTreeMap<u32, u32> = BTreeMap::new();
        let elf = ElfBytes::<LittleEndian>::minimal_parse(input)?;
        if elf.ehdr.class != Class::ELF32 {
            bail!("Not a 32-bit ELF");
        }
        if elf.ehdr.e_machine != elf::abi::EM_RISCV {
            bail!("Invalid machine type, must be RISC-V");
        }
        if elf.ehdr.e_type != elf::abi::ET_EXEC {
            bail!("Invalid ELF type, must be executable");
        }
        let entry: u32 = elf.ehdr.e_entry.try_into()?;
        if entry >= max_mem || entry % 4 != 0 {
            bail!("Invalid entrypoint");
        }
        let segments = elf
            .segments()
            .ok_or_else(|| anyhow!("Missing segment table"))?;
        if segments.len() > 256 {
            bail!("Too many program headers");
        }

        for segment in segments.iter().filter(|x| x.p_type == elf::abi::PT_LOAD) {
            let file_size: u32 = segment.p_filesz.try_into()?;
            let mem_size: u32 = segment.p_memsz.try_into()?;
            let vaddr: u32 = segment.p_vaddr.try_into()?;
            let offset: u32 = segment.p_offset.try_into()?;
            for i in (0..mem_size).step_by(4) {
                let addr = vaddr + i;
                if i >= file_size {
                    // Past the file size, all zeros.
                    image.insert(addr, 0);
                } else {
                    let mut word = 0;
                    // Don't read past the end of the file.
                    let len = std::cmp::min(file_size - i, 4);
                    for j in 0..len {
                        let offset = (offset + i + j) as usize;
                        let byte = u32::from(input[offset]);
                        word |= byte << (j * 8);
                    }
                    image.insert(addr, word);
                }
            }
        }
        Ok(Program { entry, image })
    }
}

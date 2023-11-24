use std::cmp::{max, min};
use std::collections::HashSet;
use std::iter::repeat;

use anyhow::{anyhow, ensure, Result};
use derive_more::{Deref, DerefMut};
use elf::endian::LittleEndian;
use elf::file::Class;
use elf::segment::ProgramHeader;
use elf::string_table::StringTable;
use elf::symbol::SymbolTable;
use elf::ElfBytes;
use im::hashmap::HashMap;
use itertools::{chain, iproduct, Itertools};
use serde::{Deserialize, Serialize};

use crate::decode::decode_instruction;
use crate::instruction::{DecodingError, Instruction};
use crate::util::load_u32;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MozakMemoryRegion {
    pub starting_address: u32,
    pub capacity: u32,
    pub data: Data,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MozakMemory {
    // merkle state root
    pub state_root: MozakMemoryRegion,
    // timestamp
    pub timestamp: MozakMemoryRegion,
    // io private
    pub io_tape_private: MozakMemoryRegion,
    // io public
    pub io_tape_public: MozakMemoryRegion,
}

impl Default for MozakMemory {
    fn default() -> Self {
        // These magic numbers taken from mozak-linker-script
        MozakMemory {
            state_root: MozakMemoryRegion {
                starting_address: 0x0000_0000_u32,
                capacity: 0x0000_0100_u32,
                ..Default::default()
            },
            timestamp: MozakMemoryRegion {
                starting_address: 0x0000_0100_u32,
                capacity: 0x0000_0008_u32,
                ..Default::default()
            },
            io_tape_private: MozakMemoryRegion {
                starting_address: 0x2000_0000_u32,
                capacity: 0x2000_0000_u32,
                ..Default::default()
            },
            io_tape_public: MozakMemoryRegion {
                starting_address: 0x1000_0000_u32,
                capacity: 0x1000_0000_u32,
                ..Default::default()
            },
        }
    }
}

impl From<(&[u8], &[u8])> for MozakMemory {
    // data: private, public
    fn from(data: (&[u8], &[u8])) -> Self {
        let mut mm = MozakMemory::default();
        let mut index = mm.io_tape_private.starting_address;
        data.0.iter().for_each(|e| {
            mm.io_tape_private.data.insert(index, *e);
            index += 1;
        });
        let mut index = mm.io_tape_public.starting_address;
        data.1.iter().for_each(|e| {
            mm.io_tape_public.data.insert(index, *e);
            index += 1;
        });
        mm
    }
}
impl MozakMemory {
    fn is_mozak_ro_memory_address(&self, ph: &ProgramHeader) -> bool {
        let address: u32 =
            u32::try_from(ph.p_vaddr).expect("p_vaddr for zk-vm expected to be cast-able to u32");
        let mem_addresses = [
            (
                self.state_root.starting_address,
                self.state_root.starting_address + self.state_root.capacity,
            ),
            (
                self.timestamp.starting_address,
                self.timestamp.starting_address + self.timestamp.capacity,
            ),
            (
                self.io_tape_public.starting_address,
                self.io_tape_public.starting_address + self.io_tape_public.capacity,
            ),
            (
                self.io_tape_private.starting_address,
                self.io_tape_private.starting_address + self.io_tape_private.capacity,
            ),
        ];
        log::trace!(
            "mozak-memory-addresses: {:?}, address: {:?}",
            mem_addresses,
            address
        );
        for ell in &mem_addresses {
            if (ell.0 <= address) && (address < ell.1) {
                return true;
            }
        }
        false
    }

    fn fill(&mut self, st: &(SymbolTable<LittleEndian>, StringTable)) {
        for s in st.0.iter() {
            let sym_name = st.1.get(s.st_name as usize).unwrap().to_string();
            let sym_value = s.st_value;
            log::trace!("sym_name: {:?}", sym_name);
            log::trace!("sym_value: {:0x}", sym_value);

            match sym_name.as_str() {
                "_mozak_merkle_state_root" => {
                    self.state_root.starting_address = u32::try_from(sym_value)
                        .expect("state_root address should be u32 cast-able");
                    log::debug!(
                        "_mozak_merkle_state_root: 0x{:0x}",
                        self.state_root.starting_address
                    );
                }
                "_mozak_merkle_state_root_capacity" => {
                    self.state_root.capacity = u32::try_from(sym_value)
                        .expect("state_root_max_capacity should be u32 cast-able");
                    log::debug!(
                        "_mozak_merkle_state_root_capacity: 0x{:0x}",
                        self.state_root.capacity
                    );
                }
                "_mozak_timestamp" => {
                    self.timestamp.starting_address = u32::try_from(sym_value)
                        .expect("timestamp address should be u32 cast-able");
                    log::debug!("_mozak_timestamp: 0x{:0x}", self.timestamp.starting_address);
                }
                "_mozak_timestamp_capacity" => {
                    self.timestamp.capacity = u32::try_from(sym_value)
                        .expect("timestamp_max_capacity should be u32 cast-able");
                    log::debug!(
                        "_mozak_timestamp_capacity: 0x{:0x}",
                        self.timestamp.capacity
                    );
                }
                "_mozak_public_io_tape" => {
                    self.io_tape_public.starting_address = u32::try_from(sym_value)
                        .expect("io_tape_public address should be u32 cast-able");
                    log::debug!(
                        "_mozak_public_io_tape: 0x{:0x}",
                        self.io_tape_public.starting_address
                    );
                }
                "_mozak_public_io_tape_capacity" => {
                    self.io_tape_public.capacity = u32::try_from(sym_value)
                        .expect("io_tape_public_max_capacity should be u32 cast-able");
                    log::debug!(
                        "_mozak_public_io_tape_capacity: 0x{:0x}",
                        self.io_tape_public.capacity
                    );
                }
                "_mozak_private_io_tape" => {
                    self.io_tape_private.starting_address = u32::try_from(sym_value)
                        .expect("io_tape_private address should be u32 cast-able");
                    log::debug!(
                        "_mozak_private_io_tape: 0x{:0x}",
                        self.io_tape_private.starting_address
                    );
                }
                "_mozak_private_io_tape_capacity" => {
                    self.io_tape_private.capacity = u32::try_from(sym_value)
                        .expect("io_tape_private_max_capacity should be u32 cast-able");
                    log::debug!(
                        "_mozak_private_io_tape_capacity: 0x{:0x}",
                        self.io_tape_private.capacity
                    );
                }
                _ => {}
            }
        }
    }
}

/// A Mozak program runtime arguments
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MozakRunTimeArguments {
    state_root: [u8; 32],
    timestamp: [u8; 4],
    io_tape_private: Vec<u8>,
    io_tape_public: Vec<u8>,
}

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

    /// Mozak run-time memory
    pub mozak_ro_memory: MozakMemory,
}

/// Executable code of the ELF
///
/// A wrapper of a map from pc to [Instruction]
#[derive(Clone, Debug, Default, Deref, Serialize, Deserialize)]
pub struct Code(pub HashMap<u32, Result<Instruction, DecodingError>>);

/// Memory of RISC-V Program
///
/// A wrapper around a map from a 32-bit address to a byte of memory
#[derive(Clone, Debug, Default, Deref, Serialize, Deserialize, DerefMut)]
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
            mozak_ro_memory: MozakMemory::default(),
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
    ///
    /// # Panics
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
            .segments()
            .ok_or_else(|| anyhow!("Missing segment table"))?;
        ensure!(segments.len() <= 256, "Too many program headers");

        let extract = |check_flags: fn(u32, s: &ProgramHeader, m: &MozakMemory) -> bool,
                       m: &MozakMemory| {
            segments
                .iter()
                .filter(|s| check_flags(s.p_flags, s, m))
                .map(|segment| -> anyhow::Result<_> {
                    let file_size: usize = segment.p_filesz.try_into()?;
                    let mem_size: usize = segment.p_memsz.try_into()?;
                    let vaddr: u32 = segment.p_vaddr.try_into()?;
                    let offset = segment.p_offset.try_into()?;

                    let min_size = min(file_size, mem_size);
                    let max_size = max(file_size, mem_size);

                    log::trace!(
                        "file_size: {:?}, \
                        mem_size: {:?}, \
                        vaddr: {:0x}, \
                        offset: {:?}, \
                        min_size: {:?}, \
                        max_size: {:?}",
                        file_size,
                        mem_size,
                        vaddr,
                        offset,
                        min_size,
                        max_size
                    );
                    Ok((vaddr..).zip(
                        chain!(&input[offset..][..min_size], repeat(&0u8))
                            .take(max_size)
                            .copied(),
                    ))
                })
                .flatten_ok()
                .try_collect()
        };
        let mut mozak_ro_memory: MozakMemory = MozakMemory::default();
        mozak_ro_memory.fill(&elf.symbol_table().unwrap().unwrap());

        let ro_memory = Data(extract(
            |flags, ph, mozak_memory: &MozakMemory| {
                (flags & elf::abi::PF_R == elf::abi::PF_R)
                    && (flags & elf::abi::PF_W == elf::abi::PF_NONE)
                    && (!mozak_memory.is_mozak_ro_memory_address(ph))
            },
            &mozak_ro_memory,
        )?);

        let ro_memory_addresses = ro_memory.keys().sorted().collect_vec();
        log::debug!(
            "ro_memory_addresses_start:{:#0x}, ro_memory_addresses_end: {:#0x}",
            ro_memory_addresses.first().unwrap(),
            ro_memory_addresses.last().unwrap()
        );
        let rw_memory = Data(extract(
            |flags, _, _| flags == elf::abi::PF_R | elf::abi::PF_W,
            &mozak_ro_memory,
        )?);
        let rw_memory_addresses = rw_memory.keys().sorted().collect_vec();
        log::debug!(
            "rw_memory_addresses_start:{:#0x}, rw_memory_addresses_end: {:#0x}",
            rw_memory_addresses.first().unwrap(),
            rw_memory_addresses.last().unwrap()
        );
        // Because we are implementing a modified Harvard Architecture, we make an
        // independent copy of the executable segments. In practice,
        // instructions will be in a R_X segment, so their data will show up in ro_code
        // and ro_memory. (RWX segments would show up in ro_code and rw_memory.)
        let ro_code = Code::from(&extract(
            |flags, _, _| flags & elf::abi::PF_X == elf::abi::PF_X,
            &mozak_ro_memory,
        )?);
        let ro_code_addresses = ro_code.keys().sorted().collect_vec();
        log::debug!(
            "ro_code_start:{:#0x}, ro_code_end: {:#0x}",
            ro_code_addresses.first().unwrap(),
            ro_code_addresses.last().unwrap()
        );
        Ok(Program {
            entry_point,
            ro_memory,
            rw_memory,
            ro_code,
            mozak_ro_memory,
        })
    }

    /// # Errors
    /// Will return `Err` if the ELF file is invalid or if the entrypoint is
    /// invalid.
    ///
    /// # Panics
    /// TODO: Roman
    pub fn load_program(
        elf_bytes: &[u8],
        io_tape_private: &[u8],
        io_tape_public: &[u8],
    ) -> Result<Program> {
        let mut program = Program::load_elf(elf_bytes).unwrap();
        let io_priv_start_addr = program.mozak_ro_memory.io_tape_private.starting_address;
        for (i, e) in io_tape_private.iter().enumerate() {
            program
                .mozak_ro_memory
                .io_tape_private
                .data
                .insert(io_priv_start_addr + u32::try_from(i).unwrap(), *e);
        }
        let io_pub_start_addr = program.mozak_ro_memory.io_tape_public.starting_address;
        for (i, e) in io_tape_public.iter().enumerate() {
            program
                .mozak_ro_memory
                .io_tape_public
                .data
                .insert(io_pub_start_addr + u32::try_from(i).unwrap(), *e);
        }
        Ok(program)
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

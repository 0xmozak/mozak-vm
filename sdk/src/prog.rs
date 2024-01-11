use rkyv::{Archive, Deserialize, Serialize};
use crate::coretypes::{Poseidon2HashType};

/// Each program in the mozak ecosystem is identifyable by two
/// hashes: `ProgramRomHash` & `MemoryInitHash`.
#[derive(Archive, Deserialize, Serialize, PartialEq, Default)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
#[cfg_attr(not(target_os = "zkvm"), derive(Debug))]
pub struct ProgramIdentifier {
    /// ProgramRomHash defines the hash of the text section of the
    /// static ELF program concerned
    pub program_rom_hash: Poseidon2HashType,

    /// MemoryInitHash defines the hash of the static memory initialization
    /// regions of the static ELF program concerned
    pub memory_init_hash: Poseidon2HashType,
}

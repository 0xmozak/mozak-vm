use rkyv::{Archive, Serialize, Deserialize};

/// Canonical hashed type in "mozak vm". Can store hashed values of
/// Poseidon2 hash.
#[derive(Archive, Deserialize, Serialize, PartialEq, Default)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
pub struct Poseidon2HashType([u8; 4]);

#[cfg(not(target_os = "zkvm"))]
impl std::fmt::Debug for Poseidon2HashType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Poseidon2HashType")
            .field(
                "hash",
                &self
                    .0
                    .iter()
                    .map(|x| hex::encode(x))
                    .collect::<Vec<String>>(),
            )
            .finish()
    }
}

pub const STATE_TREE_DEPTH: usize = 8;

/// Canonical "address" type of object in "mozak vm".
#[derive(Archive, Deserialize, Serialize, PartialEq, Default)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
pub struct Address([u8; STATE_TREE_DEPTH]);

#[cfg(not(target_os = "zkvm"))]
impl std::fmt::Debug for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Address")
            .field(
                "address",
                &self
                    .0
                    .iter()
                    .map(|x| hex::encode(x))
                    .collect::<Vec<String>>(),
            )
            .finish()
    }
}

/// Each program in the mozak ecosystem is identifyable by two
/// hashes: `program_rom_hash` & `memory_init_hash` and a program
/// entry point `entry_point`
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

    /// Entry point of the program
    pub entry_point: u32,
}


/// Each storage object is a unit of information in the global
/// state tree constrained for modification only by its `constraint_owner`
#[derive(Archive, Deserialize, Serialize, PartialEq, Default)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
#[cfg_attr(not(target_os = "zkvm"), derive(Debug))]
pub struct StateObject {
    /// [IMMUTABLE] Constraint-Owner is the only program which can
    /// mutate the `metadata` and `data` fields of this object
	constraint_owner: ProgramIdentifier,

	/// [MUTABLE] Object-associated Metadata (e.g. managing permissions, 
    /// expiry, etc.)
	// metadata: StateObjectMetadata,

	/// [MUTABLE] Serialized data object understandable and affectable
    /// by `constraint_owner`
	data: &[u8],
}

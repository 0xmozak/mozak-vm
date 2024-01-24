use rkyv::{Archive, Deserialize, Serialize};

/// Canonical hashed type in "mozak vm". Can store hashed values of
/// Poseidon2 hash.
#[derive(Archive, Deserialize, Serialize, PartialEq, Eq, Default)]
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

impl Poseidon2HashType {
    pub fn to_le_bytes(&self) -> [u8; 4] {
        self.0
    }
}

pub const STATE_TREE_DEPTH: usize = 8;

/// Canonical "address" type of object in "mozak vm".
#[derive(Archive, Deserialize, Serialize, PartialEq, Eq, Default)]
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

impl Address {
    pub fn get_raw(&self) -> [u8; STATE_TREE_DEPTH] { self.0 }
}

/// Each program in the mozak ecosystem is identifyable by two
/// hashes: `program_rom_hash` & `memory_init_hash` and a program
/// entry point `entry_point`
#[derive(Archive, Deserialize, Serialize, PartialEq, Eq, Default)]
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

impl ProgramIdentifier {
    /// Checks if the objects all have the same `constraint_owner` as
    /// `self`.
    pub fn ensure_constraint_owner_similarity<'a, T>(&self, objects: T)
    where
        T: Iterator<Item = &'a StateObject<'a>> + Sized, {
        objects.for_each(|x| {
            if x.constraint_owner != *self {
                panic!("constraint owner does not match program identifier");
            }
        })
    }

    pub fn to_le_bytes(&self) -> [u8; 12] {
        let mut le_bytes_array:[u8; 12] = [0; 12];
        le_bytes_array[0..4].copy_from_slice(&self.program_rom_hash.to_le_bytes());
        le_bytes_array[4..8].copy_from_slice(&self.memory_init_hash.to_le_bytes());
        le_bytes_array[8..12].copy_from_slice(&self.entry_point.to_le_bytes());
        le_bytes_array
    }
}

/// Each storage object is a unit of information in the global
/// state tree constrained for modification only by its `constraint_owner`
#[derive(Archive, Deserialize, Serialize, PartialEq, Eq, Default)]
#[archive(compare(PartialEq))]
#[cfg_attr(not(target_os = "zkvm"), derive(Debug))]
pub struct StateObject<'a> {
    /// [IMMUTABLE] Logical address of StateObject in the tree
    pub address: Address,

    /// [IMMUTABLE] Constraint-Owner is the only program which can
    /// mutate the `metadata` and `data` fields of this object
    pub constraint_owner: ProgramIdentifier,

    /// [MUTABLE] Object-associated Metadata (e.g. managing permissions,
    /// expiry, etc.)
    // metadata: StateObjectMetadata,

    /// [MUTABLE] Serialized data object understandable and affectable
    /// by `constraint_owner`
    pub data: &'a [u8],
}

// /// Inefficient, better to use 32 bit limbs
// construct_uint! {
// 	pub struct U256(4);
// }

// struct ArchivedU256 {

// }

// struct U256Resolver {

// }

// impl Archive for U256 {
//     type Archived = ArchivedU256;
//     type Resolver = U256Resolver;

//     // The resolve function consumes the resolver and produces the archived
//     // value at the given position.
//     unsafe fn resolve(
//         &self,
//         pos: usize,
//         resolver: Self::Resolver,
//         out: *mut Self::Archived,
//     ) {

//     }
// }

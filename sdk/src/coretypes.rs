use rkyv::{AlignedVec, Archive, Deserialize, Serialize};

/// Canonical hashed type in "mozak vm". Can store hashed values of
/// Poseidon2 hash.
#[derive(Archive, Deserialize, Serialize, PartialEq, Eq, Default, Copy, Clone)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
pub struct Poseidon2HashType([u8; 4]);

#[cfg(not(target_os = "zkvm"))]
impl std::ops::Deref for Poseidon2HashType {
    type Target = [u8; 4];

    fn deref(&self) -> &Self::Target { &self.0 }
}

#[cfg(not(target_os = "zkvm"))]
impl std::fmt::Debug for Poseidon2HashType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Poseidon2HashType({:?})",
            &self
                .iter()
                .map(|x| hex::encode([*x]))
                .collect::<Vec<String>>()
                .join("")
        )
    }
}

impl Poseidon2HashType {
    #[must_use]
    pub fn to_le_bytes(&self) -> [u8; 4] { self.0 }
}

impl From<[u8; 4]> for Poseidon2HashType {
    fn from(value: [u8; 4]) -> Self { Poseidon2HashType(value) }
}

pub const STATE_TREE_DEPTH: usize = 8;

/// Canonical "address" type of object in "mozak vm".
#[derive(Archive, Deserialize, Serialize, PartialEq, Eq, Default, Copy, Clone)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
pub struct Address([u8; STATE_TREE_DEPTH]);

#[cfg(not(target_os = "zkvm"))]
impl std::ops::Deref for Address {
    type Target = [u8; STATE_TREE_DEPTH];

    fn deref(&self) -> &Self::Target { &self.0 }
}

#[cfg(not(target_os = "zkvm"))]
impl std::fmt::Debug for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Addr: 0x{}",
            &self
                .iter()
                .map(|x| hex::encode([*x]))
                .collect::<Vec<String>>()
                .join(""),
        )
    }
}

impl Address {
    #[must_use]
    pub fn get_raw(&self) -> [u8; STATE_TREE_DEPTH] { self.0 }
}

impl From<[u8; STATE_TREE_DEPTH]> for Address {
    fn from(value: [u8; STATE_TREE_DEPTH]) -> Self { Address(value) }
}

/// Each program in the mozak ecosystem is identifyable by two
/// hashes: `program_rom_hash` & `memory_init_hash` and a program
/// entry point `entry_point`
#[derive(Archive, Deserialize, Serialize, PartialEq, Eq, Default, Copy, Clone)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
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
    ///
    /// # Panics
    ///
    /// Panicks if all given objects don't have the same constraint owner as
    /// `self`.
    pub fn ensure_constraint_owner_similarity<'a, T>(&self, objects: T)
    where
        T: Iterator<Item = &'a StateObject> + Sized, {
        objects.for_each(|x| {
            assert!(
                !(x.constraint_owner != *self),
                "constraint owner does not match program identifier"
            );
        });
    }

    #[must_use]
    pub fn to_le_bytes(&self) -> [u8; 12] {
        let mut le_bytes_array: [u8; 12] = [0; 12];
        le_bytes_array[0..4].copy_from_slice(&self.program_rom_hash.to_le_bytes());
        le_bytes_array[4..8].copy_from_slice(&self.memory_init_hash.to_le_bytes());
        le_bytes_array[8..12].copy_from_slice(&self.entry_point.to_le_bytes());
        le_bytes_array
    }
}

#[cfg(not(target_os = "zkvm"))]
impl std::fmt::Debug for ProgramIdentifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "MZK-{}-{}-{}",
            &self
                .program_rom_hash
                .to_le_bytes()
                .iter()
                .map(|x| hex::encode([*x]))
                .collect::<Vec<String>>()
                .join(""),
            &self
                .memory_init_hash
                .to_le_bytes()
                .iter()
                .map(|x| hex::encode([*x]))
                .collect::<Vec<String>>()
                .join(""),
            &self.entry_point,
        )
    }
}

/// Each storage object is a unit of information in the global
/// state tree constrained for modification only by its `constraint_owner`
#[derive(Archive, Deserialize, Serialize, PartialEq, Eq, Default, Clone)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
#[cfg_attr(not(target_os = "zkvm"), derive(Debug))]
pub struct StateObject {
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
    pub data: Vec<u8>,
}

#[derive(Archive, Deserialize, Serialize, PartialEq, Eq, Default, Clone)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
pub struct RawMessage(pub Vec<u8>);

#[cfg(not(target_os = "zkvm"))]
impl std::fmt::Debug for RawMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "0x{}",
            &self
                .iter()
                .map(|x| hex::encode([*x]))
                .collect::<Vec<String>>()
                .join("")
        )
    }
}

/// wrapper class over rkyv `AlignedVec`
#[cfg(not(target_os = "zkvm"))]
impl std::ops::Deref for RawMessage {
    type Target = Vec<u8>;

    fn deref(&self) -> &Self::Target { &self.0 }
}

impl From<Vec<u8>> for RawMessage {
    fn from(value: Vec<u8>) -> RawMessage { RawMessage(value) }
}

impl From<AlignedVec> for RawMessage {
    fn from(value: AlignedVec) -> RawMessage { RawMessage(value.into_vec()) }
}

/// Canonical "address" type of object in "mozak vm".
#[derive(Archive, Deserialize, Serialize, PartialEq, Eq, Default, Clone)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
#[cfg_attr(not(target_os = "zkvm"), derive(Debug))]
pub struct CPCMessage {
    /// caller of cross-program-call message. Tuple of ProgramID
    /// and methodID
    pub caller_prog: ProgramIdentifier,

    /// recipient of cross-program-call message. Tuple of ProgramID
    /// and methodID
    pub callee_prog: ProgramIdentifier,

    /// serialized call arguments
    pub args: RawMessage,

    /// serialized return value
    pub ret: RawMessage,
}

/// Context-variables access
#[derive(Archive, Deserialize, Serialize, PartialEq, Eq, Clone)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
#[cfg_attr(not(target_os = "zkvm"), derive(Debug))]
pub enum ContextVariable {
    BlockHeight(u64)
}

/// Events emitted on `Events` tape
#[derive(Archive, Deserialize, Serialize, PartialEq, Eq, Clone)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
#[cfg_attr(not(target_os = "zkvm"), derive(Debug))]
pub enum Event {
    ReadContextVariable(ContextVariable),
    ReadStateObject(StateObject),
    UpdatedStateObject(StateObject),
    CreatedStateObject(StateObject),
    DeletedStateObject(StateObject),
}

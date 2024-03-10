#[cfg(not(target_os = "mozakvm"))]
use itertools::{chain, Itertools};
use rkyv::{AlignedVec, Archive, Deserialize, Serialize};

pub const DIGEST_BYTES: usize = 32;

/// Canonical hashed type in "mozak vm". Can store hashed values of
/// Poseidon2 hash.
#[derive(
    Archive, Deserialize, Serialize, PartialEq, Eq, Default, Copy, Clone, PartialOrd, Ord, Hash,
)]
#[cfg_attr(target_os = "mozakvm", derive(Debug))]
#[cfg_attr(
    not(target_os = "mozakvm"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
pub struct Poseidon2HashType(pub [u8; DIGEST_BYTES]);

impl core::ops::Deref for Poseidon2HashType {
    type Target = [u8; DIGEST_BYTES];

    fn deref(&self) -> &Self::Target { &self.0 }
}

#[cfg(not(target_os = "mozakvm"))]
impl std::ops::Deref for Poseidon2HashType {
    type Target = [u8; DIGEST_BYTES];

    fn deref(&self) -> &Self::Target { &self.0 }
}

#[cfg(not(target_os = "mozakvm"))]
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
    pub fn to_le_bytes(&self) -> [u8; DIGEST_BYTES] { self.0 }
}

impl From<[u8; DIGEST_BYTES]> for Poseidon2HashType {
    fn from(value: [u8; DIGEST_BYTES]) -> Self { Poseidon2HashType(value) }
}

impl From<Vec<u8>> for Poseidon2HashType {
    fn from(value: Vec<u8>) -> Poseidon2HashType {
        assert_eq!(value.len(), DIGEST_BYTES);
        <&[u8] as TryInto<[u8; DIGEST_BYTES]>>::try_into(&value[0..DIGEST_BYTES])
            .expect("Vec<u8> must have exactly {DIGEST_BYTES} elements")
            .into()
    }
}

pub const STATE_TREE_DEPTH: usize = 8;

/// Canonical "address" type of object in "mozak vm".
#[derive(Archive, Deserialize, Serialize, PartialEq, Eq, Default, Copy, Clone)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
#[cfg_attr(target_os = "mozakvm", derive(Debug))]
pub struct Address([u8; STATE_TREE_DEPTH]);

#[cfg(not(target_os = "mozakvm"))]
impl std::ops::Deref for Address {
    type Target = [u8; STATE_TREE_DEPTH];

    fn deref(&self) -> &Self::Target { &self.0 }
}

#[cfg(not(target_os = "mozakvm"))]
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

/// Each program in the Mozak ecosystem is identifiable by two
/// hashes: `program_rom_hash` & `memory_init_hash` and a program
/// entry point `entry_point`
#[derive(
    Archive, Deserialize, Serialize, PartialEq, Eq, Default, Copy, Clone, PartialOrd, Ord, Hash,
)]
#[cfg_attr(
    not(target_os = "mozakvm"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(target_os = "mozakvm", derive(Debug))]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
pub struct ProgramIdentifier(pub Poseidon2HashType);

impl ProgramIdentifier {
    #[cfg(not(target_os = "mozakvm"))]
    pub fn new(
        program_rom_hash: Poseidon2HashType,
        memory_init_hash: Poseidon2HashType,
        entry_point: u32,
    ) -> Self {
        use crate::sys::poseidon2_hash;

        let input = chain!(
            program_rom_hash.to_le_bytes(),
            memory_init_hash.to_le_bytes(),
            entry_point.to_le_bytes(),
        )
        .collect_vec();

        Self(poseidon2_hash(&input))
    }

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
                x.constraint_owner == *self,
                "constraint owner does not match program identifier"
            );
        });
    }

    #[must_use]
    pub fn to_le_bytes(&self) -> [u8; DIGEST_BYTES] {
        let mut le_bytes_array: [u8; DIGEST_BYTES] = [0; DIGEST_BYTES];
        le_bytes_array[0..DIGEST_BYTES].copy_from_slice(&self.0.to_le_bytes());
        le_bytes_array
    }
}

#[cfg(not(target_os = "mozakvm"))]
impl std::fmt::Debug for ProgramIdentifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "MZK-{}",
            &self
                .to_le_bytes()
                .iter()
                .map(|x| hex::encode([*x]))
                .collect::<Vec<String>>()
                .join(""),
        )
    }
}

#[cfg(not(target_os = "mozakvm"))]
impl From<String> for ProgramIdentifier {
    fn from(value: String) -> ProgramIdentifier {
        let components: Vec<&str> = value.split("-").collect();
        assert_eq!(components.len(), 2);
        assert_eq!(components[0], "MZK");

        ProgramIdentifier(Poseidon2HashType::from(hex::decode(components[1]).unwrap()))
    }
}

/// Each storage object is a unit of information in the global
/// state tree constrained for modification only by its `constraint_owner`
#[derive(Archive, Deserialize, Serialize, PartialEq, Eq, Default, Clone)]
#[archive(compare(PartialEq))]
#[cfg_attr(target_os = "mozakvm", derive(Debug))]
#[archive_attr(derive(Debug))]
// #[cfg_attr(not(target_os = "mozakvm"), derive(Debug))]
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

#[cfg(not(target_os = "mozakvm"))]
impl std::fmt::Debug for StateObject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "({:?}, {:?}): Data: 0x{}",
            &self.address,
            &self.constraint_owner,
            &self
                .data
                .iter()
                .map(|x| hex::encode([*x]))
                .collect::<Vec<String>>()
                .join("")
        )
    }
}

#[derive(Archive, Deserialize, Serialize, PartialEq, Eq, Default, Clone)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
pub struct RawMessage(pub Vec<u8>);

#[cfg(not(target_os = "mozakvm"))]
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

impl core::ops::Deref for RawMessage {
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
#[cfg_attr(not(target_os = "mozakvm"), derive(Debug))]
pub struct CPCMessage {
    /// caller of cross-program-call message. Tuple of ProgramID
    /// and methodID
    /// TODO: Think about correctness of this??
    pub caller_prog: ProgramIdentifier,

    /// recipient of cross-program-call message. Tuple of ProgramID
    /// and methodID
    pub callee_prog: ProgramIdentifier,

    /// raw message over cpc
    pub args: RawMessage,
    pub ret: RawMessage,
}

#[derive(Archive, Deserialize, Serialize, PartialEq, Eq, Default, Clone)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
pub struct Signature(Vec<u8>);

#[cfg(not(target_os = "mozakvm"))]
impl std::fmt::Debug for Signature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Signature({:?})",
            &self
                .iter()
                .map(|x| hex::encode([*x]))
                .collect::<Vec<String>>()
                .join("")
        )
    }
}

#[cfg(not(target_os = "mozakvm"))]
impl std::ops::Deref for Signature {
    type Target = Vec<u8>;

    fn deref(&self) -> &Self::Target { &self.0 }
}

impl From<Vec<u8>> for Signature {
    fn from(value: Vec<u8>) -> Signature { Signature(value) }
}

#[derive(Archive, Debug, Deserialize, Serialize, PartialEq, Eq, Clone)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
pub enum ContextVariable {
    BlockHeight(u64),
    SelfProgramIdentifier(ProgramIdentifier),
}

#[derive(Archive, Debug, Deserialize, Serialize, PartialEq, Eq, Clone)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
pub enum Event {
    ReadContextVariable(ContextVariable),
    ReadStateObject(StateObject),
    UpdatedStateObject(StateObject),
    CreatedStateObject(StateObject),
    DeletedStateObject(StateObject),
}

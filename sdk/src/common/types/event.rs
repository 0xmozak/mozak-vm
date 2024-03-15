#[derive(
    Copy,
    Clone,
    Hash,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
// Derives only for non-mozakvm
#[cfg_attr(
    not(target_os = "mozakvm"),
    derive(Debug, serde::Serialize, serde::Deserialize)
)]
#[allow(clippy::module_name_repetitions)]
#[repr(u8)]
pub enum EventType {
    Read = 0,
    Write,
    Ensure,
    Create,
    Delete,
}

impl Default for EventType {
    fn default() -> Self { Self::Read }
}

// Common derives
#[derive(
    Default, Clone, Hash, PartialEq, PartialOrd, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize,
)]
// Derives only for non-mozakvm
#[cfg_attr(
    not(target_os = "mozakvm"),
    derive(Debug, serde::Serialize, serde::Deserialize)
)]
pub struct Event {
    pub object: super::StateObject,
    pub type_: EventType,
}

// Common derives
#[derive(
    Default,
    Copy,
    Clone,
    Hash,
    PartialEq,
    PartialOrd,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
// Derives only for non-mozakvm
#[cfg_attr(
    not(target_os = "mozakvm"),
    derive(Debug, serde::Serialize, serde::Deserialize)
)]
#[allow(clippy::module_name_repetitions)]
pub struct CanonicalEvent {
    pub address: super::StateAddress,
    pub type_: EventType,
    pub value: super::Poseidon2Hash,
    pub emitter: super::ProgramIdentifier,
}

#[cfg(not(target_os = "mozakvm"))]
#[allow(dead_code)]
impl CanonicalEvent {
    fn from_event(emitter: super::ProgramIdentifier, value: &Event) -> Self {
        Self {
            address: value.object.address,
            type_: value.type_,
            value: crate::native::helpers::poseidon2_hash(&value.object.data),
            emitter,
        }
    }
}

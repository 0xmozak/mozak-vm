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

#[derive(
    Default, Clone, Hash, PartialEq, PartialOrd, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize,
)]
#[cfg_attr(
    not(target_os = "mozakvm"),
    derive(Debug, serde::Serialize, serde::Deserialize)
)]
pub struct Event {
    pub object: super::StateObject,
    pub type_: EventType,
}

#[derive(
    Default,
    Copy,
    Clone,
    Hash,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
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

impl CanonicalEvent {
    #[must_use]
    pub fn from_event(emitter: super::ProgramIdentifier, value: &Event) -> Self {
        #[cfg(not(target_os = "mozakvm"))]
        {
            Self {
                address: value.object.address,
                type_: value.type_,
                value: crate::native::helpers::poseidon2_hash(&value.object.data),
                emitter,
            }
        }
        #[cfg(target_os = "mozakvm")]
        {
            Self {
                address: value.object.address,
                type_: value.type_,
                value: crate::mozakvm::helpers::poseidon2_hash(&value.object.data),
                emitter,
            }
        }
    }
}

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
#[cfg_attr(
    not(target_os = "mozakvm"),
    derive(Debug, serde::Serialize, serde::Deserialize)
)]
pub struct CanonicalOrderedTemporalHints(pub CanonicalEvent, pub u32);

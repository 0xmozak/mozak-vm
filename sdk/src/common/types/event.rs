use crate::common::types::state_address::STATE_TREE_DEPTH;
use crate::core::constants::DIGEST_BYTES;
#[cfg(target_os = "mozakvm")]
use crate::mozakvm::poseidon::poseidon2_hash_no_pad;
#[cfg(not(target_os = "mozakvm"))]
use crate::native::poseidon::poseidon2_hash_no_pad;
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
    Write = 0,
    Ensure,
    Read,
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
}

impl CanonicalEvent {
    #[must_use]
    pub fn from_event(value: &Event) -> Self {
        #[cfg(not(target_os = "mozakvm"))]
        {
            Self {
                address: value.object.address,
                type_: value.type_,
                value: crate::native::poseidon::poseidon2_hash_with_pad(&value.object.data),
            }
        }
        #[cfg(target_os = "mozakvm")]
        {
            Self {
                address: value.object.address,
                type_: value.type_,
                value: crate::mozakvm::poseidon::poseidon2_hash_with_pad(&value.object.data),
            }
        }
    }

    #[must_use]
    pub fn canonical_hash(&self) -> super::poseidon2hash::Poseidon2Hash {
        const U64_LEN: usize = 0u64.to_be_bytes().len();
        const LEN: usize = U64_LEN + STATE_TREE_DEPTH + DIGEST_BYTES;
        let data_to_hash: [u8; LEN] = array_concat::concat_arrays!(
            u64::from(self.type_ as u8).to_le_bytes(),
            self.address.inner(),
            self.value.inner()
        );
        poseidon2_hash_no_pad(&data_to_hash)
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

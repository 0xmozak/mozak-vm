#[cfg(not(target_os = "mozakvm"))]
use serde_hex::{SerHexSeq, StrictPfx};

// Common derives
#[derive(
    Default, Clone, Hash, PartialEq, PartialOrd, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize,
)]
// Derives only for non-mozakvm
#[cfg_attr(
    not(target_os = "mozakvm"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct RawMessage(
    #[cfg_attr(not(target_os = "mozakvm"), serde(with = "SerHexSeq::<StrictPfx>"))] pub Vec<u8>,
);

#[cfg(not(target_os = "mozakvm"))]
impl std::fmt::Debug for RawMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "0x{}",
            &self.iter().map(|x| hex::encode([*x])).collect::<String>()
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

impl From<rkyv::AlignedVec> for RawMessage {
    fn from(value: rkyv::AlignedVec) -> RawMessage { RawMessage(value.into_vec()) }
}

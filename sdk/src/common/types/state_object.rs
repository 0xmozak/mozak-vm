#[cfg(not(target_os = "mozakvm"))]
use serde_hex::{SerHexSeq, StrictPfx};

#[derive(
    Default, Clone, Hash, PartialEq, PartialOrd, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize,
)]
#[cfg_attr(
    not(target_os = "mozakvm"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[archive(check_bytes)]
pub struct StateObject {
    pub address: super::StateAddress,
    pub constraint_owner: super::ProgramIdentifier,
    #[cfg_attr(not(target_os = "mozakvm"), serde(with = "SerHexSeq::<StrictPfx>"))]
    pub data: Vec<u8>,
}

#[cfg(not(target_os = "mozakvm"))]
impl std::fmt::Debug for StateObject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "({:?} owned by {:?}) => 0x{}",
            &self.address,
            &self.constraint_owner,
            &self
                .data
                .iter()
                .map(|x| hex::encode([*x]))
                .collect::<String>()
        )
    }
}

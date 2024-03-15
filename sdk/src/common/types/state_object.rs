// Common derives
#[derive(
    Default, Clone, Hash, PartialEq, PartialOrd, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize,
)]
// Derives only for non-mozakvm
#[cfg_attr(
    not(target_os = "mozakvm"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct StateObject {
    pub address: super::StateAddress,
    pub constraint_owner: super::ProgramIdentifier,
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

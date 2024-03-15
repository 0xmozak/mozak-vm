pub const STATE_TREE_DEPTH: usize = 4;

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
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct StateAddress([u8; STATE_TREE_DEPTH]);

impl std::ops::Deref for StateAddress {
    type Target = [u8; STATE_TREE_DEPTH];

    fn deref(&self) -> &Self::Target { &self.0 }
}

#[cfg(not(target_os = "mozakvm"))]
impl std::fmt::Debug for StateAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "StateAddress: 0x{}",
            &self.iter().map(|x| hex::encode([*x])).collect::<String>()
        )
    }
}

impl StateAddress {
    #[must_use]
    pub fn inner(self) -> [u8; STATE_TREE_DEPTH] { self.0 }
}

impl From<[u8; STATE_TREE_DEPTH]> for StateAddress {
    fn from(value: [u8; STATE_TREE_DEPTH]) -> StateAddress { StateAddress(value) }
}

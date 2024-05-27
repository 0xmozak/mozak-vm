#[cfg(not(target_os = "mozakvm"))]
use serde_hex::{SerHex, StrictPfx};

pub const STATE_TREE_DEPTH: usize = 8;

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
    derive(serde::Serialize, serde::Deserialize)
)]
#[archive(check_bytes)]
pub struct StateAddress(
    #[cfg_attr(not(target_os = "mozakvm"), serde(with = "SerHex::<StrictPfx>"))]
    pub  [u8; STATE_TREE_DEPTH],
);

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

    #[must_use]
    #[cfg(not(target_os = "mozakvm"))]
    pub fn new_from_rand_seed(seed: u64) -> Self {
        use rand::prelude::*;
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(seed);
        let mut slice: [u8; STATE_TREE_DEPTH] = [0; STATE_TREE_DEPTH];
        rng.fill_bytes(&mut slice[..]);
        Self(slice)
    }
}

impl From<[u8; STATE_TREE_DEPTH]> for StateAddress {
    fn from(value: [u8; STATE_TREE_DEPTH]) -> StateAddress { StateAddress(value) }
}

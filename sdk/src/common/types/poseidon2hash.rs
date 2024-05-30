#[cfg(not(target_os = "mozakvm"))]
use serde_hex::{SerHex, StrictPfx};

use crate::core::constants::DIGEST_BYTES;

#[derive(
    Clone,
    Copy,
    Default,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    rkyv::Archive,
    rkyv::Deserialize,
    rkyv::Serialize,
)]
#[archive(check_bytes)]
#[cfg_attr(
    not(target_os = "mozakvm"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[archive_attr(derive(Debug))]
pub struct Poseidon2Hash(
    #[cfg_attr(not(target_os = "mozakvm"), serde(with = "SerHex::<StrictPfx>"))]
    pub  [u8; DIGEST_BYTES],
);

impl core::ops::Deref for Poseidon2Hash {
    type Target = [u8; DIGEST_BYTES];

    fn deref(&self) -> &Self::Target { &self.0 }
}

#[cfg(not(target_os = "mozakvm"))]
impl std::fmt::Debug for Poseidon2Hash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Poseidon2Hash(0x{:?})",
            &self.iter().map(|x| hex::encode([*x])).collect::<String>()
        )
    }
}

impl Poseidon2Hash {
    #[must_use]
    pub fn inner(&self) -> [u8; DIGEST_BYTES] { self.0 }

    pub fn to_u64s(&self) -> [u64; 4] {
        let mut bytes = [[0; 8]; DIGEST_BYTES / 8];
        array_util::flatten_mut(&mut bytes).copy_from_slice(&self.0);
        bytes.map(u64::from_le_bytes)
    }

    #[must_use]
    pub fn two_to_one(l: Self, r: Self) -> Self {
        #[cfg(target_os = "mozakvm")]
        use crate::mozakvm::poseidon::poseidon2_hash_no_pad;
        #[cfg(not(target_os = "mozakvm"))]
        use crate::native::poseidon::poseidon2_hash_no_pad;

        poseidon2_hash_no_pad(array_util::flatten(&[l.0, r.0]))
    }

    #[must_use]
    #[cfg(not(target_os = "mozakvm"))]
    pub fn new_from_rand_seed(seed: u64) -> Self {
        use rand::prelude::*;
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(seed);
        let mut slice: [u8; DIGEST_BYTES] = [0; DIGEST_BYTES];
        rng.fill_bytes(&mut slice[..]);
        Self(slice)
    }
}

impl From<[u8; DIGEST_BYTES]> for Poseidon2Hash {
    fn from(value: [u8; DIGEST_BYTES]) -> Self { Poseidon2Hash(value) }
}

impl From<[u64; 4]> for Poseidon2Hash {
    fn from(value: [u64; 4]) -> Self {
        let flat_vec: Vec<u8> = value.into_iter().flat_map(u64::to_le_bytes).collect();
        flat_vec.into()
    }
}

impl From<Vec<u8>> for Poseidon2Hash {
    fn from(value: Vec<u8>) -> Poseidon2Hash {
        assert_eq!(value.len(), DIGEST_BYTES);
        <&[u8] as TryInto<[u8; DIGEST_BYTES]>>::try_into(&value[0..DIGEST_BYTES])
            .expect("Vec<u8> must have exactly {DIGEST_BYTES} elements")
            .into()
    }
}

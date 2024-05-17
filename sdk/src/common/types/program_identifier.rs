use crate::core::constants::DIGEST_BYTES;
#[cfg(target_os = "mozakvm")]
use crate::mozakvm::helpers::poseidon2_hash_with_pad;
#[cfg(not(target_os = "mozakvm"))]
use crate::native::helpers::poseidon2_hash_with_pad;

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
#[cfg_attr(
    not(target_os = "mozakvm"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct ProgramIdentifier(pub super::Poseidon2Hash);

impl ProgramIdentifier {
    #[must_use]
    pub fn new(
        program_hash: super::Poseidon2Hash,
        memory_init_hash: super::Poseidon2Hash,
        entry_point: u32,
    ) -> Self {
        let input: Vec<u8> = itertools::chain!(
            program_hash.inner(),
            memory_init_hash.inner(),
            entry_point.to_le_bytes(),
        )
        .collect();
        // would be of length 32 + 32 + 4 = 68. And 68 % 8 !=0
        // Hence would require padding, before being hashed
        Self(poseidon2_hash_with_pad(&input))
    }

    #[must_use]
    #[cfg(not(target_os = "mozakvm"))]
    pub fn new_from_rand_seed(seed: u64) -> Self {
        Self(super::Poseidon2Hash::new_from_rand_seed(seed))
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
        T: Iterator<Item = &'a super::StateObject> + Sized, {
        objects.for_each(|x| {
            assert!(
                x.constraint_owner == *self,
                "constraint owner does not match program identifier"
            );
        });
    }

    #[must_use]
    pub fn inner(&self) -> [u8; DIGEST_BYTES] {
        let mut le_bytes_array: [u8; DIGEST_BYTES] = [0; DIGEST_BYTES];
        le_bytes_array[0..DIGEST_BYTES].copy_from_slice(&self.0.inner());
        le_bytes_array
    }

    #[must_use]
    /// Checks if `self` is the null program, i.e. the program with ID
    /// `MZK-000000000000000000000000000000000000000000000000000000000000000`
    pub fn is_null_program(&self) -> bool { self == &Self::default() }
}

#[cfg(not(target_os = "mozakvm"))]
impl std::fmt::Debug for ProgramIdentifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "MZK-{}",
            &self
                .0
                 .0
                .iter()
                .map(|x| hex::encode([*x]))
                .collect::<String>(),
        )
    }
}

#[cfg(not(target_os = "mozakvm"))]
impl From<String> for ProgramIdentifier {
    fn from(value: String) -> ProgramIdentifier {
        let components: Vec<&str> = value.split('-').collect();
        assert_eq!(components.len(), 2);
        assert_eq!(components[0], "MZK");

        ProgramIdentifier(super::Poseidon2Hash::from(
            hex::decode(components[1]).unwrap(),
        ))
    }
}

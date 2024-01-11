use rkyv::{Archive, Serialize, Deserialize};

/// Canonical hashed type in "mozak vm". Can store hashed values of
/// Poseidon2 hash.
#[derive(Archive, Deserialize, Serialize, PartialEq, Default)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
pub struct Poseidon2HashType([u8; 4]);

#[cfg(not(target_os = "zkvm"))]
impl std::fmt::Debug for Poseidon2HashType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Poseidon2HashType")
            .field(
                "hash",
                &self
                    .0
                    .iter()
                    .map(|x| hex::encode(x))
                    .collect::<Vec<String>>(),
            )
            .finish()
    }
}

pub const STATE_TREE_DEPTH: usize = 8;

/// Canonical "address" type of object in "mozak vm".
#[derive(Archive, Deserialize, Serialize, PartialEq, Default)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
pub struct Address([u8; STATE_TREE_DEPTH]);

#[cfg(not(target_os = "zkvm"))]
impl std::fmt::Debug for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Address")
            .field(
                "address",
                &self
                    .0
                    .iter()
                    .map(|x| hex::encode(x))
                    .collect::<Vec<String>>(),
            )
            .finish()
    }
}

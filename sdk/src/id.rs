use std::fmt::{self, Display};
use std::str;

use derive_more::Deref;
use serde::{Deserialize, Serialize};
use sha3::Digest;

use crate::crypto::DefaultHash;

/// ID is a unique identifier for any part of the system.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Hash, Serialize, Deserialize, Deref)]
pub struct Id(pub [u8; 32]);

impl Id {
    /// Creates a new ID from a byte slice.
    #[must_use]
    pub fn new(id: &[u8]) -> Self {
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(id);
        Self(bytes)
    }

    /// Derive ID from another id and a seed.
    #[must_use]
    pub fn derive(id: Id, seed: u64) -> Self {
        let mut hasher = DefaultHash::new();
        hasher.update(id.to_vec());
        hasher.update(seed.to_le_bytes());
        let hash = hasher.finalize();
        Id(hash.into())
    }

    /// Create random ID for testing
    #[must_use]
    pub fn random() -> Self {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let mut bytes = [0u8; 32];
        rng.fill(&mut bytes);
        Self(bytes)
    }
}

impl Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            str::from_utf8(&self.0).expect("ID should be valid UTF-8 bytes")
        )
    }
}

pub trait Indexable {
    fn id(&self) -> Id;
}

use derive_more::Deref;
#[cfg(all(feature = "dummy-system", feature = "std"))]
use rand::distributions::Standard;
#[cfg(all(feature = "dummy-system", feature = "std"))]
use rand::prelude::Distribution;
#[cfg(all(feature = "dummy-system", feature = "std"))]
use rand::Rng;
use serde::{Deserialize, Serialize};

/// ID is a unique identifier for any part of the system.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Hash, Serialize, Deserialize, Deref)]
pub struct Id(pub [u8; 32]);

#[cfg(all(feature = "dummy-system", feature = "std"))]
impl Distribution<Id> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Id { Id(rng.gen()) }
}

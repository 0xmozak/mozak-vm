use std::ops::Deref;

use rand::distributions::Standard;
use rand::prelude::Distribution;
use rand::Rng;
use serde::{Deserialize, Serialize};

/// ID is a unique identifier for any part of the system.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct Id(pub [u8; 32]);

impl Deref for Id {
    type Target = [u8; 32];

    fn deref(&self) -> &Self::Target { &self.0 }
}

#[cfg(feature = "dummy-system")]
impl Distribution<Id> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Id { Id(rng.gen()) }
}

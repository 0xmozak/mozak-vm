use rand::distributions::Standard;
use rand::prelude::Distribution;
use rand::Rng;

/// ID is a unique identifier for any part of the system.
#[derive(Debug, Default, Clone)]
pub struct Id {
    pub id: [u32; 8],
}

#[cfg(feature = "dummy-server")]
impl Distribution<Id> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Id {
        Id {
            id: rng.gen(),
        }
    }
}

pub mod merkle;
pub mod system;
pub(crate) mod traits;
pub mod types;

pub mod constants {
    pub const DIGEST_BYTES: usize = crate::common::types::poseidon2hash::DIGEST_BYTES;
}

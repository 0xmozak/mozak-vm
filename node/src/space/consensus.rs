use thiserror::Error;

use crate::space::blobs::Blob;
use crate::space::storage::SpaceStorage;
use crate::stark::StarkProof;

pub trait ConsensusSystem {
    fn initiate() -> Self;

    /// Pushes a state update with proof to the consensus system.
    /// `updated_blobs` and `read_blobs` are the public inputs to the STARK
    /// proof.
    /// TODO - use hashes of changed and read blobs to reduce the public inputs.
    fn push_state_updates(
        &mut self,
        updated_blobs: Vec<Blob>,
        read_blobs: Vec<Blob>,
        proof: StarkProof,
    ) -> Result<(), ConsensusError>;

    /// Fetches the latest state that we have reached consensus on
    fn fetch_last_settled_state(&self) -> &SpaceStorage;
}

#[derive(Error, Debug)]
pub enum ConsensusError {
    #[error("Proof Verification Failed")]
    IncorrectProof,
    #[error("Other Error")]
    OtherError(String),
}

/// We could also consider to use the Dummy Consensus as internal consensus
/// mechanism, so that while waiting for the real consensus to be settled, we
/// can continue to process and execute messages.
#[cfg(feature = "dummy-system")]
pub struct DummyConsensusSystem {
    storage: SpaceStorage,
}

#[cfg(feature = "dummy-system")]
impl ConsensusSystem for DummyConsensusSystem {
    fn initiate() -> Self {
        Self {
            storage: SpaceStorage::initiate(),
        }
    }

    fn push_state_updates(
        &mut self,
        updated_blobs: Vec<Blob>,
        _read_blobs: Vec<Blob>,
        _proof: StarkProof,
    ) -> Result<(), ConsensusError> {
        self.storage.update_blobs(updated_blobs);

        // TODO - check the update proof here

        Ok(())
    }

    fn fetch_last_settled_state(&self) -> &SpaceStorage { &self.storage }
}

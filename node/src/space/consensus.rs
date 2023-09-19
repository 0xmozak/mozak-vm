use thiserror::Error;

use crate::proof::ProgramRunProof;
use crate::space::object::Object;
use crate::space::storage::ApplicationStorage;

pub trait ConsensusSystem {
    fn initiate() -> Self;

    /// Pushes a state update with proof to the consensus system.
    /// `updated_blobs` and `read_blobs` are the public inputs to the STARK
    /// proof.
    /// TODO - use hashes of changed and read blobs to reduce the public inputs.
    fn push_state_updates(
        &mut self,
        updated_blobs: Vec<Object>,
        read_blobs: Vec<Object>,
        proof: ProgramRunProof,
    ) -> Result<(), ConsensusError>;

    /// Fetches the latest state that we have reached consensus on
    fn fetch_last_settled_state(&self) -> &ApplicationStorage;
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
    storage: ApplicationStorage,
}

#[cfg(feature = "dummy-system")]
impl ConsensusSystem for DummyConsensusSystem {
    fn initiate() -> Self {
        Self {
            storage: ApplicationStorage::initiate(),
        }
    }

    fn push_state_updates(
        &mut self,
        updated_blobs: Vec<Object>,
        _read_blobs: Vec<Object>,
        _proof: ProgramRunProof,
    ) -> Result<(), ConsensusError> {
        self.storage.update_objects(updated_blobs);

        // TODO - check the update proof here

        Ok(())
    }

    fn fetch_last_settled_state(&self) -> &ApplicationStorage { &self.storage }
}

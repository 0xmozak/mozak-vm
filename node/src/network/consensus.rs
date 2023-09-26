use thiserror::Error;

use crate::network::storage::ApplicationStorage;
use crate::proof::{verify_block_transition_proof, BlockTransitionWithProof};
use crate::Object;

pub trait ConsensusSystem {
    fn initiate(root_object: Object) -> Self;

    /// Pushes a state update with proof to the consensus system.
    /// `updated_blobs` and `read_blobs` are the public inputs to the STARK
    /// proof.
    /// TODO - use hashes of changed and read blobs to reduce the public inputs.
    fn push_block_update(
        &mut self,
        block_transition: BlockTransitionWithProof,
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
    fn initiate(root_object: Object) -> Self {
        Self {
            storage: ApplicationStorage::initiate(root_object),
        }
    }

    /// Pushes a state update with proof to the consensus system.
    /// For now, we will not verify the proof, but just update the state.
    fn push_block_update(
        &mut self,
        block_transition: BlockTransitionWithProof,
    ) -> Result<(), ConsensusError> {
        let updated_objects = block_transition.changed_objects;

        #[allow(clippy::all)]
        verify_block_transition_proof(block_transition.proof)
            .map_err(|_| ConsensusError::IncorrectProof)?;

        self.storage.update_objects(updated_objects);

        // TODO - check the update proof here

        Ok(())
    }

    fn fetch_last_settled_state(&self) -> &ApplicationStorage { &self.storage }
}

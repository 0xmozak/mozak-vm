#![feature(assert_matches)]

pub use id::Id;
pub use network::consensus::{ConsensusSystem, DummyConsensusSystem};
pub use network::object::Object;
pub use network::storage::ApplicationStorage;
pub use node::message::TransitionMessage;
pub use node::rpc::{DummyRPC, ScenarioRPC, RPC};
pub use proof::{
    batch_batched_transition_proof, batch_transition_proofs, prove_transition_function,
    TransitionWithProof,
};
pub use sequencer::Sequencer;
pub use vm::run_transition_function;

/// Module that handles node interactions with the node.
mod node;

mod id;

/// Module that contains the network management logic.
mod network;
mod proof;
mod sequencer;
mod vm;

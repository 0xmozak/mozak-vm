#![feature(assert_matches)]

pub use id::Id;
pub use proof::ProgramRunProof;
pub use rpc::message::TransitionMessage;
pub use rpc::rpc::{DummyRPC, RPC};
pub use space::consensus::{ConsensusSystem, DummyConsensusSystem};
pub use space::object::Object;
pub use space::storage::ApplicationStorage;
pub use vm::{prove_transition_function, run_transition_function};

/// Module that handles rpc interactions with the node.
mod rpc;

mod id;

mod proof;
/// Module that contains the space management logic.
mod space;
mod vm;

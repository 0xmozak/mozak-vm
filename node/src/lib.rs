#![feature(assert_matches)]

pub use id::Id;
pub use proof::ProgramRunProof;
pub use rpc::message::{Argument, Message};
pub use rpc::message_service::{DummyMessageService, MessageService};
pub use space::blobs::Blob;
pub use space::consensus::{ConsensusSystem, DummyConsensusSystem};
pub use space::storage::SpaceStorage;
pub use vm::{run_program, ELF};

/// Module that handles rpc interactions with the node.
mod rpc;

mod id;

mod proof;
/// Module that contains the space management logic.
mod space;
mod vm;

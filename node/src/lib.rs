#![feature(assert_matches)]

pub use id::Id;
pub use space::blobs::Blob;
pub use space::consensus::{ConsensusSystem, DummyConsensusSystem};
pub use space::storage::SpaceStorage;
pub use stark::StarkProof;
pub use user::message::Message;
pub use user::message_service::{DummyMessageService, MessageService};

/// Module that handles user interactions with the node.
mod user;

mod id;

/// Module that contains the space management logic.
mod space;
mod stark;

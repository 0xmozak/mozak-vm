use rand::distributions::Standard;
use rand::prelude::Distribution;
use rand::{Rng, RngCore};

use crate::rpc::message::Argument::U32;
use crate::Id;

/// A raw Message data passed from the clients to the node. This will be parsed
/// into a [Message].
/// TODO - use a serialization format like protobuf or bincode.
pub struct RawMessage {
    data: Vec<u8>,
}

impl From<RawMessage> for Message {
    fn from(#[allow(unused_variables)] message: RawMessage) -> Self { unimplemented!() }
}

/// Message
#[derive(Debug, Clone)]
pub struct Message {
    pub target_program: Id,
    pub inputs: Vec<Argument>,
}

impl Message {
    pub fn destruct(self) -> (Id, Vec<Argument>) { (self.target_program, self.inputs) }
}

/// Supported types of inputs
/// We Support what the RISC-V supports
/// Though we can add more types for convenience and readability
#[derive(Debug, Clone)]
pub enum Argument {
    U32(u32),
    U32Array(Vec<u32>),
}

#[cfg(feature = "dummy-system")]
impl Distribution<Message> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Message {
        Message {
            target_program: Id([0; 32]),
            inputs: vec![U32(rng.next_u32())],
        }
    }
}

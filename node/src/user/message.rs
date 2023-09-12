use rand::distributions::Standard;
use rand::prelude::Distribution;
use rand::Rng;

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

/// Currently we allow for a fix set of Messages.
/// In the future, we will support a more general Message format, where a client
/// can pass a list of arbitrary arguments to a program.
#[derive(Debug, Clone)]
pub enum Message {
    Transfer { from: Id, to: Id, amount: u64 },
}

#[cfg(feature = "dummy-system")]
impl Distribution<Message> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Message {
        Message::Transfer {
            from: rng.gen(),
            to: rng.gen(),
            amount: rng.gen(),
        }
    }
}

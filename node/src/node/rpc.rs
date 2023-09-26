use rand::{Rng, RngCore, SeedableRng};

use crate::node::message::TransitionMessage;

/// The service that is responsible for handling the action messages received
/// from the client.
pub trait RPC {
    /// Creates a new service instance.
    fn new() -> Self;

    /// Returns the next message to be processed.
    /// TODO - make function async.
    fn get_next_message(&mut self) -> Option<TransitionMessage>;
}

/// RPC that generates random messages.
#[cfg(feature = "dummy-system")]
pub struct DummyRPC {
    rng: Box<dyn RngCore>,
}

#[cfg(feature = "dummy-system")]
impl RPC for DummyRPC {
    fn new() -> Self {
        // Set up the random number generator with a fixed seed.
        let rng = rand::rngs::StdRng::from_seed([0; 32]);
        let rng = Box::new(rng);

        Self { rng }
    }

    fn get_next_message(&mut self) -> Option<TransitionMessage> { Some(self.rng.gen()) }
}

/// RPC that receives a scenario of messages and then returns them one by one.
#[cfg(feature = "dummy-system")]
pub struct ScenarioRPC {
    messages: Vec<TransitionMessage>,
}

#[cfg(feature = "dummy-system")]
impl RPC for ScenarioRPC {
    fn new() -> Self {
        let messages = vec![];
        Self { messages }
    }

    fn get_next_message(&mut self) -> Option<TransitionMessage> { self.messages.pop() }
}

#[cfg(feature = "dummy-system")]
impl ScenarioRPC {
    /// Batch imports a scenario into the RPC.
    pub fn import_scenario(messages: Vec<TransitionMessage>) -> Self { Self { messages } }

    /// Adds a message to the RPC scenario queue.
    pub fn add_message(&mut self, message: TransitionMessage) { self.messages.push(message); }
}

#[cfg(test)]
mod tests {
    use std::assert_matches::assert_matches;

    use super::*;

    #[test]
    fn test_dummy_message_service() {
        let mut service = DummyRPC::new();
        let message = service.get_next_message();
        assert_matches!(message, Some(TransitionMessage { .. }));
    }
}

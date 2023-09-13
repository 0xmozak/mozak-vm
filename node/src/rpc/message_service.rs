use rand::prelude::Distribution;
use rand::{Rng, RngCore, SeedableRng};

use crate::rpc::message::Message;

/// The service that is responsible for handling the action messages received
/// from the client.
pub trait MessageService {
    /// Creates a new service instance.
    fn new() -> Self;

    /// Returns the next message to be processed.
    /// TODO - make function async.
    fn get_next_message(&mut self) -> Message;
}

#[cfg(feature = "dummy-system")]
pub struct DummyMessageService {
    rng: Box<dyn RngCore>,
}

#[cfg(feature = "dummy-system")]
impl MessageService for DummyMessageService {
    fn new() -> Self {
        // Set up the random number generator with a fixed seed.
        let rng = rand::rngs::StdRng::from_seed([0; 32]);
        let rng = Box::new(rng);

        Self { rng }
    }

    fn get_next_message(&mut self) -> Message { self.rng.gen() }
}

#[cfg(test)]
mod tests {
    use std::assert_matches::assert_matches;

    use super::*;

    #[test]
    fn test_dummy_message_service() {
        let mut service = DummyMessageService::new();
        let message = service.get_next_message();
        assert_matches!(message, Message { .. });
    }
}

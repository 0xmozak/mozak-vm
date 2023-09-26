use node::{ConsensusSystem, DummyConsensusSystem, DummyRPC, Object, Sequencer, RPC};

#[cfg(feature = "dummy-system")]
fn main() {
    let root_object = Object::default();

    let mut network = DummyConsensusSystem::initiate(root_object);
    // Initiate a new message service that will receive messages from clients.
    let mut message_service = DummyRPC::new();

    Sequencer::run(&mut network, &mut message_service);
}

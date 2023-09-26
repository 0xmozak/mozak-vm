use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::network::object::TransitionFunction;
use crate::{
    batch_batched_transition_proof, batch_transition_proofs, prove_transition_function,
    run_transition_function, ConsensusSystem, Object, TransitionWithProof, RPC,
};

pub struct Sequencer();

impl Sequencer {
    pub fn run(network: &mut impl ConsensusSystem, message_service: &mut impl RPC) {
        let mut latest_storage_state = network.fetch_last_settled_state();

        let mut object_updates = HashMap::new();
        let mut pending_transitions = Vec::new();

        loop {
            // 1. Get the next client Message
            let message = message_service.get_next_message();

            // If there is no message, we break the loop and stop the sequencer.
            let message = match message {
                None => {
                    break;
                }
                Some(message) => message,
            };

            // 2. Start a thread to work with the transaction
            // TODO - multithread it
            let transition_with_proof = {
                // 1. Get the Transition Function that will be validated from the Storage
                let program = latest_storage_state
                    .get_object(message.owner_program_id)
                    .unwrap()
                    .as_program()
                    .unwrap();

                let transition_function = program
                    .allowed_transitions
                    .get(&message.target_transition_id)
                    .unwrap();

                // 2. Get actual objects from the Storage
                let read_objects: Vec<Object> = message
                    .read_objects_id
                    .iter()
                    .map(|id| latest_storage_state.get_object(*id).unwrap().clone())
                    .collect();

                let changed_objects_before: Vec<Object> = message
                    .changed_objects
                    .iter()
                    .map(|object| {
                        latest_storage_state
                            .get_object(object.id())
                            .unwrap()
                            .clone()
                    })
                    .collect();
                let changed_objects_after = &message.changed_objects;

                // 3. Check that the read object has not been proposed to change since the last
                //    state update. If it has, we then reject the transaction.

                read_objects.iter().for_each(|object| {
                    if object_updates.contains_key(&object.id()) {

                        // If the key is present, this transaction is blocked and should wait for the next state update.
                        panic!("Transaction blocked by another transaction that is changing the same object");
                    }
                });

                // 4. Check the transition to be satisfied in the RISC-V VM, before doing hard
                //    work of proving it
                run_transition_function(
                    transition_function,
                    &read_objects,
                    &changed_objects_before,
                    changed_objects_after,
                    &message.input,
                )
                .unwrap();

                #[allow(clippy::all)]
                // 5. Prove that the transition was run correctly in the RISC-V VM
                let transition_proof = prove_transition_function(
                    transition_function,
                    &read_objects,
                    &changed_objects_before,
                    changed_objects_after,
                    &message.input,
                )
                .unwrap();

                TransitionWithProof {
                    transition_id: message.target_transition_id,
                    read_objects_id: message.read_objects_id,
                    changed_objects: message.changed_objects,
                    proof: transition_proof,
                }
            };
            // 6. Add states to the list of changed states, and their associated proofs
            transition_with_proof
                .changed_objects
                .iter()
                .for_each(|object| {
                    object_updates.insert(object.id(), pending_transitions.len());
                });
            pending_transitions.push(transition_with_proof);

            // 3. Once in a while, collect the state updates and try to squash them. We can
            //    only squash state updates that don't conflict with each other (whose
            //    updated_states do not exist in the read_states of other updates). We pop
            //    these merged states from the state_updates vector. If we have a conflict,
            //    we will have to process this transaction again with   new state as input.
            //    It also means we have done the proof work for nothing,   but as the design
            //    of the system encourages parallelism, this should almost  never happen.

            if pending_transitions.len() < 10 {
                continue;
            }

            let batched_transition_proof = batch_transition_proofs(&pending_transitions);

            let block_transition_proof =
                batch_batched_transition_proof([batched_transition_proof].as_slice());

            // 4. We push the merged state updates to the consensus system
            network.push_block_update(block_transition_proof).unwrap();
            // 5. We update the state of the network with the merged state updates. All
            //    state updates proofs must now be based on this state.
            latest_storage_state = network.fetch_last_settled_state();
        }
    }

    /// Loads a transition function from an ELF file on disk.
    #[allow(dead_code)] // TODO - remove this
    fn load_transition_from_file<P: AsRef<Path>>(
        path: P,
    ) -> Result<TransitionFunction, &'static str> {
        println!("Current directory: {:?}", std::env::current_dir().unwrap());

        let mut file = File::open(path).map_err(|_| "Could not open file")?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).unwrap();

        let transition =
            TransitionFunction::load_elf(buffer.as_slice()).map_err(|_| "Could not load ELF")?;

        Ok(transition)
    }
}

#[cfg(all(feature = "dummy-system", test))]
mod test {
    use crate::network::object::program::{generate_transition_id, ProgramContent};
    use crate::sequencer::Sequencer;
    use crate::{
        ConsensusSystem, DummyConsensusSystem, Id, Object, ScenarioRPC, TransitionMessage, RPC,
    };

    #[test]
    fn no_message_test() {
        let mut network = DummyConsensusSystem::initiate(Object::default());

        let mut message_service = ScenarioRPC::new();

        Sequencer::run(&mut network, &mut message_service);
    }

    #[test]
    fn single_message_test() -> Result<(), &'static str> {
        let yes_man_transition =
            Sequencer::load_transition_from_file("../transitions/yes_man/yes_man_transition")?;
        let root_object = {
            Object::Program(ProgramContent::new(0, false, Id([0u8; 32]), vec![
                yes_man_transition.clone(),
            ]))
        };
        let root_object_id = root_object.id();

        let mut network = DummyConsensusSystem::initiate(root_object);

        let mut message_service = {
            let mut rpc = ScenarioRPC::new();

            // We do not do any state transition, but just push a message to the network.
            let message_1 = TransitionMessage {
                owner_program_id: root_object_id,
                target_transition_id: generate_transition_id(&yes_man_transition),
                read_objects_id: Vec::new(),
                changed_objects: Vec::new(),
                input: Vec::new(),
            };

            rpc.add_message(message_1);

            rpc
        };

        Sequencer::run(&mut network, &mut message_service);

        Ok(())
    }
}

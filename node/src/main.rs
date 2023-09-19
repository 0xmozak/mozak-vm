use std::collections::HashMap;

use node::{
    prove_transition_function, run_transition_function, ConsensusSystem, DummyConsensusSystem,
    DummyRPC, Object, RPC,
};

use crate::placeholder::*;

#[cfg(feature = "dummy-system")]
fn main() {
    // Initiate a new message service that will receive messages from clients.
    let mut message_service = DummyRPC::new();

    let mut network = DummyConsensusSystem::initiate();

    let mut latest_storage_state = network.fetch_last_settled_state();

    let mut object_updates = HashMap::new();
    let mut pending_transitions = Vec::new();

    loop {
        // 1. Get the next client Message
        let message = message_service.get_next_message();

        // 2. Start a thread to work with the transaction
        // TODO - multithread it
        let (updated_states, viewed_states, input, update_proof) = {
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
                .read_objects
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
            let changed_objects_after = message.changed_objects;

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
                &changed_objects_after,
                &message.input,
            )
            .unwrap();

            // 5. Prove that the transition was run correctly in the RISC-V VM
            let transition_proof = prove_transition_function(
                transition_function,
                &read_objects,
                &changed_objects_before,
                &changed_objects_after,
                &message.input,
            )
            .unwrap();

            (
                read_objects,
                changed_objects_after,
                message.input,
                transition_proof,
            )
        };
        // 6. Add states to the list of changed states, and their associated proofs
        updated_states.iter().for_each(|object| {
            object_updates.insert(object.id(), pending_transitions.len());
        });
        pending_transitions.push((
            updated_states,
            viewed_states,
            input,
            update_proof,
            message.target_transition_id,
        ));

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

        let (merged_state_updates, merged_read_states, merged_proof) =
            merge_state_updates(&pending_transitions);

        // 4. We push the merged state updates to the consensus system
        network
            .push_state_updates(merged_state_updates, merged_read_states, merged_proof)
            .unwrap();
        // 5. We update the state of the space with the merged state updates. All state
        //    updates proofs must now be based on this state.
        latest_storage_state = network.fetch_last_settled_state();
    }
}

#[allow(unused_variables)]
mod placeholder {
    use node::{Id, Object, ProgramRunProof};

    pub fn merge_state_updates(
        p0: &Vec<(Vec<Object>, Vec<Object>, Vec<u8>, (), Id)>,
    ) -> (Vec<Object>, Vec<Object>, ProgramRunProof) {
        unimplemented!()
    }
}

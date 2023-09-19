use node::{run_program, Argument, ConsensusSystem, DummyConsensusSystem, DummyRPC, Id, RPC};

use crate::placeholder::*;

#[cfg(feature = "dummy-system")]
fn main() {
    // Initiate a new message service that will receive messages from clients.
    let mut message_service = DummyRPC::new();

    let mut network = DummyConsensusSystem::initiate();

    let mut latest_state = network.fetch_last_settled_state();

    let mut state_updates = Vec::new();

    loop {
        // 1. Get the next client Message
        let message = message_service.get_next_message();

        // 2. Start a thread to work with the transaction
        // TODO - multithread it
        let (updated_states, viewed_states, update_proof) = {
            // 1. Obtain target program and program arguments from the message
            let (program_id, program_input): (Id, &Vec<Argument>) = (Id::default(), &Vec::new()); // parse_transaction(message);
                                                                                                  // 2. Get the Program from the Program Manager
            let program = latest_state
                .get_object(program_id)
                .unwrap()
                .as_program()
                .unwrap();

            // 3. Run the Program in the RISC-V VM
            let (output, read_states, updated_states) = run_program(
                &program.accepted_transitions[0].ro_code,
                &program_input,
                latest_state,
            );
            // 4. Prove that the Program was run correctly in the RISC-V VM
            let (update_proof) = prove_program_run(
                (), // program
                &program_input,
                output,
                &read_states,
                &updated_states,
            );

            (updated_states, read_states, update_proof)
        };
        // 5. Update used states of the space
        state_updates.push((updated_states, viewed_states, update_proof));

        // 3. Once in a while, collect the state updates and try to squash them. We can
        //    only squash state updates that don't conflict with each other (whose
        //    updated_states do not exist in the read_states of other updates). We pop
        //    these merged states from the state_updates vector. If we have a conflict,
        //    we will have to process this transaction again with   new state as input.
        //    It also means we have done the proof work for nothing,   but as the design
        //    of the system encourages parallelism, this should almost  never happen.
        let (merged_state_updates, merged_read_states, merged_proof) =
            merge_state_updates(&state_updates);

        // 4. We push the merged state updates to the consensus system
        network
            .push_state_updates(merged_state_updates, merged_read_states, merged_proof)
            .unwrap();
        // 5. We update the state of the space with the merged state updates. All state
        //    updates proofs must now be based on this state.
        latest_state = network.fetch_last_settled_state();
    }
}

#[allow(unused_variables)]
mod placeholder {
    use node::{Argument, Id, Message, Object, ProgramRunProof};

    pub fn merge_state_updates(
        p0: &Vec<(Vec<Object>, Vec<Object>, ())>,
    ) -> (Vec<Object>, Vec<Object>, ProgramRunProof) {
        unimplemented!()
    }

    pub fn prove_program_run(
        p0: (),
        p1: &Vec<Argument>,
        p2: Vec<Argument>,
        p3: &Vec<Object>,
        p4: &Vec<Object>,
    ) -> () {
        unimplemented!()
    }

    pub fn parse_transaction(p0: Message) -> ((), (), ()) { unimplemented!() }

    pub fn get_program(p0: Id) -> () { unimplemented!() }

    pub fn get_next_transaction() -> () { unimplemented!() }
}

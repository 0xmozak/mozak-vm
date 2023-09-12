use node::{DummyMessageService, MessageService};

use crate::placeholder::*;

#[allow(unused_variables)]
#[cfg(feature = "dummy-server")]
fn main() {
    // Initiate a new message service that will receive messages from clients.
    let mut message_service = DummyMessageService::new();

    let network = Space::new();
    let space_state = SpaceStates::new();
    let mut state_updates = Vec::new();

    loop {
        // 1. Get the next client Message
        let message = message_service.get_next_message();

        // 2. Start a thread to work with the transaction
        state_updates.push(handle_transaction(message, &space_state, |message| {
            // 1. Parse the Transaction, get the program id and args
            let (program, function, arguments) = parse_transaction(message);
            // 2. Get the Program from the Program Manager
            let program = get_program(program);
            // 3. Run the Program in the RISC-V VM
            let (read_states, updated_states) = run_program(program, function, arguments);
            // 4. Prove that the Program was run correctly in the RISC-V VM
            let update_proof =
                prove_transaction(program, function, arguments, read_states, updated_states);
            // 5. Update the state of the space
            ((), (), ())
        }));

        // 3. Once in a while, collect the state updates and try to squash them. We can
        //    only squash state updates that don't conflict with each other (whose
        //    updated_states do not exist in the read_states of other updates). We pop
        //    these merged states from the state_updates vector. If we have a conflict,
        //    we will have to process this transaction again with   new state as input.
        //    It also means we have done the proof work for nothing,   but as the design
        //    of the system encourages parallelism, this should almost  never happen.
        let merged_state_updates = merge_state_updates(&state_updates);

        // 4. We push the merged state updates to the consensus system
        network.push_state_updates(merged_state_updates);
        // 5. We update the state of the space with the merged state updates. All state
        //    updates proofs must now be based on this state.
        space_state.update(merged_state_updates);
    }
}

#[allow(unused_variables)]
mod placeholder {
    use node::Message;

    pub fn merge_state_updates(p0: &Vec<((), (), ())>) -> () { unimplemented!() }

    pub struct SpaceStates {
        state: (),
    }

    pub struct Space {
        state: SpaceStates,
    }

    impl Space {
        pub fn new() -> Self { unimplemented!() }

        pub fn push_state_updates(&self, p0: ()) -> () { unimplemented!() }
    }

    impl SpaceStates {
        pub fn new() -> Self { unimplemented!() }

        pub fn update(&self, p0: ()) -> () { unimplemented!() }
    }

    pub fn run_program(p0: (), p1: (), p2: ()) -> ((), ()) { unimplemented!() }

    pub fn prove_transaction(p0: (), p1: (), p2: (), p3: (), p4: ()) -> () { unimplemented!() }

    pub fn parse_transaction(p0: Message) -> ((), (), ()) { unimplemented!() }

    pub fn get_program(p0: ()) -> () { unimplemented!() }

    pub fn handle_transaction(
        p0: Message,
        state: &SpaceStates,
        p1: fn(m: Message) -> ((), (), ()),
    ) -> ((), (), ()) {
        unimplemented!()
    }

    pub fn get_next_transaction() -> () { unimplemented!() }
}

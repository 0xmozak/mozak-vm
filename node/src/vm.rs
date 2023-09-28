use std::fmt::Error;

use flexbuffers::FlexbufferSerializer;
use mozak_node_sdk::{Object, Transition};
pub use mozak_runner::elf::Code;
use mozak_runner::elf::Program;
use mozak_runner::state::State;
use mozak_runner::vm::step;
use serde::{Deserialize, Serialize};

/// Executes the VM instance on the provided program and returns the output of
/// the program as well as updated states.
///
/// We do not support programs that make calls to other programs yet.
/// Though we can recursively call this function to support that.
#[allow(unused_variables)] // TODO - remove
pub fn run_transition_function(
    transition_function: &Transition,
    read_objects: &[Object],
    changed_objects_before: &[Object],
    changed_objects_after: &[Object],
    inputs: &[u8],
) -> Result<(), Error> {
    let vm_input = prepare_vm_input(
        read_objects,
        changed_objects_before,
        changed_objects_after,
        inputs,
    );

    // Execute the VM instance based on the input

    // TODO - provide input_bytes as input to the VM

    let transition_function = Program::from(transition_function.program.clone());

    let state = State::from(transition_function.clone());
    let state = step(&transition_function, state).unwrap().last_state;

    // TODO - check that the state has not reverted

    Ok(())
}

#[derive(Serialize, Deserialize)]
struct TransitionFunctionInput {
    read_objects: Vec<Object>,
    changed_objects_before: Vec<Object>,
    changed_objects_after: Vec<Object>,
    input: Vec<u8>,
}

/// We use the Flex-buffer serialisation to convert transition function inputs
/// into a byte array
pub(super) fn prepare_vm_input(
    read_objects: &[Object],
    changed_objects_before: &[Object],
    changed_objects_after: &[Object],
    inputs: &[u8],
) -> Vec<u8> {
    let input = TransitionFunctionInput {
        read_objects: Vec::from(read_objects),
        changed_objects_before: Vec::from(changed_objects_before),
        changed_objects_after: Vec::from(changed_objects_after),
        input: Vec::from(inputs),
    };
    let mut serializer = FlexbufferSerializer::new();
    input.serialize(&mut serializer).unwrap();
    let serialized_input = serializer.view();

    serialized_input.to_vec()
}

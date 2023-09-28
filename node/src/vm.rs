use std::fmt::Error;

use mozak_node_sdk::{Transition, TransitionInput};
pub use mozak_runner::elf::Code;
use mozak_runner::elf::Program;
use mozak_runner::state::State;
use mozak_runner::vm::step;
use postcard::to_vec;

/// Executes the VM instance on the provided program and returns the output of
/// the program as well as updated states.
///
/// We do not support programs that make calls to other programs yet.
/// Though we can recursively call this function to support that.
#[allow(unused_variables)] // TODO - remove
pub fn run_transition_function(
    transition_function: &Transition,
    transition_input: &TransitionInput,
) -> Result<(), Error> {
    let vm_input = to_vec(transition_input).unwrap();

    // Execute the VM instance based on the input

    // TODO - provide input_bytes as input to the VM

    let transition_function = Program::from(transition_function.program.clone());

    let state = State::from(transition_function.clone());
    let state = step(&transition_function, state).unwrap().last_state;

    // TODO - check that the state has not reverted

    Ok(())
}

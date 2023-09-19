use std::fmt::Error;

pub use mozak_runner::elf::Code;

use crate::space::object::{Object, TransitionFunction};

/// Executes the VM instance on the provided program and returns the output of
/// the program as well as updated states.
///
/// We do not support programs that make calls to other programs yet.
/// Though we can recursively call this function to support that.
#[allow(unused_variables)] // TODO - remove
pub fn run_transition_function(
    transition_function: &TransitionFunction,
    read_objects: &Vec<Object>,
    changed_objects_before: &Vec<Object>,
    changed_objects_after: &Vec<Object>,
    inputs: &Vec<u8>,
) -> Result<(), Error> {
    // Execute the VM instance here and return the updated state
    // We will need to convert from the Message Input to the VM Input format

    // TODO - run VM

    Ok(())
}

#[allow(unused_variables)] // TODO - remove
pub fn prove_transition_function(
    transition_function: &TransitionFunction,
    read_objects: &Vec<Object>,
    changed_objects_before: &Vec<Object>,
    changed_objects_after: &Vec<Object>,
    inputs: &Vec<u8>,
) -> Result<(), Error> {
    // TODO - Run mozak prover

    Ok(())
}

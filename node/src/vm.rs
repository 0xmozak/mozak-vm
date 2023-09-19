use std::fmt::Error;

use flexbuffers::FlexbufferSerializer;
use mozak_circuits::stark::mozak_stark::{MozakStark, PublicInputs};
use mozak_circuits::stark::proof::AllProof;
use mozak_circuits::stark::prover::prove;
use mozak_circuits::test_utils::{standard_faster_config, C, D, F};
pub use mozak_runner::elf::Code;
use mozak_runner::state::State;
use mozak_runner::vm::step;
use plonky2::field::goldilocks_field::GoldilocksField;
use plonky2::field::types::Field;
use plonky2::util::timing::TimingTree;
use serde::{Deserialize, Serialize};

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
    let vm_input = prepare_vm_input(
        read_objects,
        changed_objects_before,
        changed_objects_after,
        inputs,
    );

    // Execute the VM instance based on the input

    // TODO - provide input_bytes as input to the VM

    let state = State::from(transition_function);
    let state = step(transition_function, state).unwrap().last_state;

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

#[allow(unused_variables)] // TODO - remove
pub fn prove_transition_function(
    transition_function: &TransitionFunction,
    read_objects: &Vec<Object>,
    changed_objects_before: &Vec<Object>,
    changed_objects_after: &Vec<Object>,
    inputs: &Vec<u8>,
) -> Result<AllProof<GoldilocksField, C, 2>, Error> {
    let vm_input = prepare_vm_input(
        read_objects,
        changed_objects_before,
        changed_objects_after,
        inputs,
    );

    // Execute the VM instance based on the input

    // TODO - provide input_bytes as input to the VM

    let state = State::from(transition_function);
    let record = step(transition_function, state).unwrap();

    #[cfg(feature = "dummy-system")]
    let stark = MozakStark::default_debug();

    #[cfg(not(feature = "dummy-system"))]
    let stark = MozakStark::default();

    let public_inputs = PublicInputs {
        entry_point: F::from_canonical_u32(transition_function.entry_point),
    };
    let all_proof = prove::<F, C, D>(
        &transition_function,
        &record,
        &stark,
        &standard_faster_config(),
        public_inputs,
        &mut TimingTree::default(),
    )?;

    Ok(all_proof)
}

/// We use the Flex-buffer serialisation to convert transition function inputs
/// into a byte array
fn prepare_vm_input(
    read_objects: &Vec<Object>,
    changed_objects_before: &Vec<Object>,
    changed_objects_after: &Vec<Object>,
    inputs: &Vec<u8>,
) -> Vec<u8> {
    let input = TransitionFunctionInput {
        read_objects: read_objects.clone(),
        changed_objects_before: changed_objects_before.clone(),
        changed_objects_after: changed_objects_after.clone(),
        input: inputs.clone(),
    };
    let mut serializer = FlexbufferSerializer::new();
    input.serialize(&mut serializer).unwrap();
    let serialized_input = serializer.view();

    serialized_input.to_vec()
}

pub fn merge_transition_proofs(
    transitions_with_proofs: &Vec<(
        Vec<Object>,
        Vec<Object>,
        Vec<u8>,
        AllProof<GoldilocksField, C, 2>,
        Id,
    )>,
) {
}

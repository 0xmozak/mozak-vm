use std::fmt::Error;

use mozak_circuits::stark::mozak_stark::{MozakStark, PublicInputs};
#[allow(unused_imports)] // TODO - remove
use mozak_circuits::stark::proof::AllProof;
#[allow(unused_imports)] // TODO - remove
use mozak_circuits::stark::verifier::verify_proof;
use mozak_circuits::test_utils::{D, F, S};
use mozak_node_sdk::{Id, Object, Transition};
use mozak_runner::elf::Program;
use mozak_runner::state::State;
use mozak_runner::vm::step;
#[allow(unused_imports)] // TODO - remove
use plonky2::field::goldilocks_field::GoldilocksField;
use plonky2::field::types::Field;

use crate::vm::prepare_vm_input;

/// Proof that a transition function was executed correctly.
pub type TransitionProof = (); // TODO - replace with `AllProof<GoldilocksField, C, 2>` once we have prover
                               // working

/// Proof that a batch of transition functions was executed correctly.
/// Can be used by user to generate a transition with private inputs, or to
/// batch dependant / independent transitions.
pub type BatchedTransitionProof = TransitionProof; // TODO - replace with actual type. For now it is the same as TransitionProof,
                                                   // as we do not support batching transition proofs yet.

/// Proof that a new block was created correctly based on all the transitions.
/// Built from a batch of batched transition proofs.
pub type BlockTransitionProof = BatchedTransitionProof; // TODO - replace with actual type. For now it is the same as
                                                        // BatchedTransitionProof, as we do not support batching batched transition
                                                        // proofs yet.

/// A transition with a proof that it was executed correctly.
/// We do not include the inputs to the transition function as they are private.
#[allow(dead_code)] // TODO - remove
pub struct TransitionWithProof {
    pub transition_id: Id,
    pub read_objects_id: Vec<Id>,
    pub changed_objects: Vec<Object>,
    pub proof: TransitionProof,
}

/// A batched transition proof, created by merging multiple transition proofs.
#[allow(dead_code)] // TODO - remove
pub struct BatchedTransitionsWithProof {
    transition_ids: Vec<Id>,
    read_objects_id: Vec<Object>,
    changed_objects: Vec<Object>,
    proof: BatchedTransitionProof,
}

/// A block transition, created by merging multiple batched transition proofs.
#[allow(dead_code)] // TODO - remove
pub struct BlockTransitionWithProof {
    pub(crate) transition_ids: Vec<Id>,
    pub(crate) read_objects_id: Vec<Object>,
    pub(crate) changed_objects: Vec<Object>,
    pub(crate) proof: BlockTransitionProof,
}

#[allow(unused_variables)] // TODO - remove
pub fn prove_transition_function(
    transition_function: &Transition,
    read_objects: &[Object],
    changed_objects_before: &[Object],
    changed_objects_after: &[Object],
    inputs: &[u8],
) -> Result<TransitionProof, &'static str> {
    let vm_input = prepare_vm_input(
        read_objects,
        changed_objects_before,
        changed_objects_after,
        inputs,
    );

    // Execute the VM instance based on the input

    // TODO - provide input_bytes as input to the VM

    // TODO - implement AsRef<Program> for Transition
    let elf_program: Program = transition_function.program.clone().into();
    let entry_point = elf_program.entry_point;

    let state = State::from(elf_program.clone());
    let record = step(&elf_program, state).unwrap();

    #[cfg(feature = "dummy-system")]
    let stark = MozakStark::<F, D>::default_debug();

    #[cfg(not(feature = "dummy-system"))]
    let stark = MozakStark::default();

    let public_inputs = PublicInputs {
        entry_point: F::from_canonical_u32(entry_point),
    };
    // TODO - uncomment once we have prover working
    // let all_proof = prove::<F, C, D>(
    //     transition_function,
    //     &record,
    //     &stark,
    //     &standard_faster_config(),
    //     public_inputs,
    //     &mut TimingTree::default(),
    // )
    // .map_err(|err| format!("Failed to generate the proof: {}",
    // err.to_string()))?;

    // TODO - Change to `Ok(all_proof)` once we have prover working
    #[allow(clippy::all)]
    Ok(TransitionProof::default())
}

/// Function that verifies the proof of a transition function.
#[allow(dead_code)] // TODO - remove
#[allow(unused_variables)] // TODO - remove
pub fn verify_transition_function_proof(proof: TransitionProof) -> Result<(), Error> {
    // TODO - uncomment once we have prover working
    // let stark = S::default();

    // verify_proof(stark, proof, &standard_faster_config()).unwrap();

    Ok(())
}

/// Function that merges multiple transition proofs into a single proof.
/// Done for optimisation.
#[allow(unused_variables)] // TODO - remove
pub fn batch_transition_proofs(
    transitions_with_proofs: &[TransitionWithProof],
) -> BatchedTransitionsWithProof {
    unimplemented!()
}

/// Function that verifies the proof of a batched transition.
#[allow(dead_code)] // TODO - remove
#[allow(unused_variables)] // TODO - remove
pub fn verify_batched_transition_proof(proof: BatchedTransitionProof) -> Result<(), Error> {
    let stark = S::default();

    // TODO - add code to verify the proof

    Ok(())
}

/// This function merges batch of transition proofs into a single proof.
/// Done for optimisation as well as to allow users to generate a transition
/// with private inputs.
#[allow(unused_variables)] // TODO - remove
pub fn batch_batched_transition_proof(
    transitions_with_proofs: &[BatchedTransitionsWithProof],
) -> BlockTransitionWithProof {
    unimplemented!()
}

/// Function that verifies the proof of a block transition.
#[allow(unused_variables)] // TODO - remove
pub fn verify_block_transition_proof(proof: BlockTransitionProof) -> Result<(), Error> {
    let stark = S::default();

    // TODO - add code to verify the proof

    Ok(())
}

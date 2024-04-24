//! Circuits for proving events can be accumulated to a state delta object.

use anyhow::Result;
use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::witness::PartialWitness;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::{CircuitConfig, CircuitData};
use plonky2::plonk::config::{AlgebraicHasher, GenericConfig};
use plonky2::plonk::proof::ProofWithPublicInputs;

use crate::subcircuits::unpruned::PartialAllowed;
use crate::subcircuits::{unbounded, unpruned};
use crate::{hash_event, Event, EventType};

// The core subcircuit for this circuit
pub mod core;

pub struct LeafCircuit<F, C, const D: usize>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>, {
    /// The recursion subcircuit
    pub unbounded: unbounded::LeafSubCircuit,

    /// The rp-style merkle hash of all event fields
    pub event_hash: unpruned::LeafSubCircuit,

    /// The event-to-state/partial-object translator
    pub partial_state: core::LeafSubCircuit,

    pub circuit: CircuitData<F, C, D>,
}

impl<F, C, const D: usize> LeafCircuit<F, C, D>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    <C as GenericConfig<D>>::Hasher: AlgebraicHasher<F>,
{
    #[must_use]
    pub fn new(circuit_config: &CircuitConfig) -> Self {
        let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());

        let unbounded_inputs = unbounded::SubCircuitInputs::default(&mut builder);
        let event_hash_inputs = unpruned::SubCircuitInputs::default(&mut builder);
        let partial_state_inputs = core::SubCircuitInputs::default(&mut builder);

        let unbounded_targets = unbounded_inputs.build_leaf::<F, C, D>(&mut builder);
        let event_hash_targets = event_hash_inputs.build_leaf(&mut builder);
        let partial_state_targets = partial_state_inputs.build_leaf(&mut builder);

        let event_hash_calc = hash_event(
            &mut builder,
            partial_state_targets.event_owner,
            partial_state_targets.event_ty,
            partial_state_targets.inputs.address,
            partial_state_targets.event_value,
        );
        builder.connect_hashes(event_hash_calc, event_hash_targets.inputs.unpruned_hash);

        let circuit = builder.build();

        let public_inputs = &circuit.prover_only.public_inputs;
        let unbounded = unbounded_targets.build(public_inputs);
        let event_hash = event_hash_targets.build(public_inputs);
        let partial_state = partial_state_targets.build(public_inputs);

        Self {
            unbounded,
            event_hash,
            partial_state,
            circuit,
        }
    }

    pub fn prove(
        &self,
        branch: &BranchCircuit<F, C, D>,
        address: u64,
        event_owner: [F; 4],
        event_ty: EventType,
        event_value: [F; 4],
    ) -> Result<ProofWithPublicInputs<F, C, D>> {
        let mut inputs = PartialWitness::new();
        self.unbounded.set_witness(&mut inputs, &branch.circuit);
        self.partial_state.set_witness(&mut inputs, Event {
            owner: event_owner,
            ty: event_ty,
            address,
            value: event_value,
        });
        self.circuit.prove(inputs)
    }
}

pub struct BranchCircuit<F, C, const D: usize>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>, {
    /// The recursion subcircuit
    pub unbounded: unbounded::BranchSubCircuit<D>,

    /// The rp-style merkle hash of all event fields
    pub event_hash: unpruned::BranchSubCircuit<PartialAllowed>,

    /// The event-to-state/partial-object translator
    pub partial_state: core::BranchSubCircuit,

    pub circuit: CircuitData<F, C, D>,
}

impl<F, C, const D: usize> BranchCircuit<F, C, D>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    <C as GenericConfig<D>>::Hasher: AlgebraicHasher<F>,
{
    #[must_use]
    pub fn new(circuit_config: &CircuitConfig, leaf: &LeafCircuit<F, C, D>) -> Self {
        let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());

        let unbounded_inputs = unbounded::SubCircuitInputs::default(&mut builder);
        let event_hash_inputs = unpruned::SubCircuitInputs::default(&mut builder);
        let partial_state_inputs = core::SubCircuitInputs::default(&mut builder);

        let unbounded_targets =
            unbounded_inputs.build_branch(&mut builder, &leaf.unbounded, &leaf.circuit);
        let event_hash_targets = event_hash_inputs.build_extended_branch(
            &mut builder,
            &leaf.event_hash.indices,
            &unbounded_targets.left_proof,
            &unbounded_targets.right_proof,
            false,
        );
        let partial_state_targets = partial_state_inputs.build_branch(
            &mut builder,
            &leaf.partial_state.indices,
            &unbounded_targets.left_proof,
            &unbounded_targets.right_proof,
        );

        builder.connect(
            event_hash_targets.extension.partial.target,
            partial_state_targets.partial.target,
        );

        let circuit = builder.build();

        let public_inputs = &circuit.prover_only.public_inputs;
        let unbounded = unbounded_targets.build(&leaf.unbounded, public_inputs);
        let event_hash = event_hash_targets.build(&leaf.event_hash.indices, public_inputs);
        let partial_state = partial_state_targets.build(&leaf.partial_state.indices, public_inputs);

        Self {
            unbounded,
            event_hash,
            partial_state,
            circuit,
        }
    }

    pub fn prove(
        &self,
        left_is_leaf: bool,
        left_proof: &ProofWithPublicInputs<F, C, D>,
        right_proof: Option<(bool, &ProofWithPublicInputs<F, C, D>)>,
    ) -> Result<ProofWithPublicInputs<F, C, D>> {
        let mut inputs = PartialWitness::new();
        let partial = right_proof.is_none();
        let (right_is_leaf, right_proof) = right_proof.unwrap_or((left_is_leaf, left_proof));
        self.unbounded.set_witness(
            &mut inputs,
            left_is_leaf,
            left_proof,
            right_is_leaf,
            right_proof,
        );
        self.event_hash.set_witness(&mut inputs, None, partial);
        if partial {
            self.partial_state
                .set_witness_from_proof(&mut inputs, &left_proof.public_inputs);
        } else {
            self.partial_state.set_witness_from_proofs(
                &mut inputs,
                &left_proof.public_inputs,
                &right_proof.public_inputs,
            );
        }
        self.circuit.prove(inputs)
    }
}

#[cfg(test)]
pub mod test {
    use lazy_static::lazy_static;
    use plonky2::field::types::Field;

    use super::*;
    use crate::test_utils::{C, CONFIG, D, F};

    lazy_static! {
        pub static ref LEAF: LeafCircuit<F, C, D> = LeafCircuit::new(&CONFIG);
        pub static ref BRANCH: BranchCircuit<F, C, D> = BranchCircuit::new(&CONFIG, &LEAF);
    }

    #[test]
    fn verify_leaf() -> Result<()> {
        let program_hash_1 = [4, 8, 15, 16].map(F::from_canonical_u64);
        let program_hash_2 = [2, 3, 4, 2].map(F::from_canonical_u64);

        let non_zero_val_1 = [3, 1, 4, 15].map(F::from_canonical_u64);
        let non_zero_val_2 = [42, 0, 0, 0].map(F::from_canonical_u64);
        let non_zero_val_3 = [42, 0, 0, 1].map(F::from_canonical_u64);

        let proof = LEAF.prove(
            &BRANCH,
            200,
            program_hash_1,
            EventType::Write,
            non_zero_val_1,
        )?;
        LEAF.circuit.verify(proof)?;

        let proof = LEAF.prove(
            &BRANCH,
            200,
            program_hash_1,
            EventType::Read,
            non_zero_val_1,
        )?;
        LEAF.circuit.verify(proof)?;

        let proof = LEAF.prove(
            &BRANCH,
            200,
            program_hash_1,
            EventType::Ensure,
            non_zero_val_1,
        )?;
        LEAF.circuit.verify(proof)?;

        let proof = LEAF.prove(
            &BRANCH,
            200,
            program_hash_1,
            EventType::GiveOwner,
            program_hash_2,
        )?;
        LEAF.circuit.verify(proof)?;

        let proof = LEAF.prove(
            &BRANCH,
            200,
            program_hash_2,
            EventType::TakeOwner,
            program_hash_1,
        )?;
        LEAF.circuit.verify(proof)?;

        let proof = LEAF.prove(
            &BRANCH,
            200,
            program_hash_1,
            EventType::CreditDelta,
            non_zero_val_2,
        )?;
        LEAF.circuit.verify(proof)?;

        let proof = LEAF.prove(
            &BRANCH,
            200,
            program_hash_1,
            EventType::CreditDelta,
            non_zero_val_3,
        )?;
        LEAF.circuit.verify(proof)?;

        Ok(())
    }

    #[test]
    fn verify_branch() -> Result<()> {
        let program_hash_1 = [4, 8, 15, 16].map(F::from_canonical_u64);

        let non_zero_val_1 = [3, 1, 4, 15].map(F::from_canonical_u64);
        let non_zero_val_2 = [1, 6, 180, 33].map(F::from_canonical_u64);

        let read_proof = LEAF.prove(
            &BRANCH,
            200,
            program_hash_1,
            EventType::Read,
            non_zero_val_1,
        )?;
        LEAF.circuit.verify(read_proof.clone())?;

        let write_proof = LEAF.prove(
            &BRANCH,
            200,
            program_hash_1,
            EventType::Write,
            non_zero_val_2,
        )?;
        LEAF.circuit.verify(write_proof.clone())?;

        let ensure_proof = LEAF.prove(
            &BRANCH,
            200,
            program_hash_1,
            EventType::Ensure,
            non_zero_val_2,
        )?;
        LEAF.circuit.verify(ensure_proof.clone())?;

        let branch_proof_1 = BRANCH.prove(true, &read_proof, Some((true, &write_proof)))?;
        BRANCH.circuit.verify(branch_proof_1.clone())?;

        let branch_proof_2 = BRANCH.prove(false, &branch_proof_1, Some((true, &ensure_proof)))?;
        BRANCH.circuit.verify(branch_proof_2.clone())?;

        let branch_proof_3 = BRANCH.prove(false, &branch_proof_2, None)?;
        BRANCH.circuit.verify(branch_proof_3)?;

        let branch_proof_4 = BRANCH.prove(true, &read_proof, None)?;
        BRANCH.circuit.verify(branch_proof_4)?;

        Ok(())
    }
}

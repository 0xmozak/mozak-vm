use anyhow::Result;
use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::target::BoolTarget;
use plonky2::iop::witness::{PartialWitness, WitnessWrite};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::{CircuitConfig, CircuitData};
use plonky2::plonk::config::{AlgebraicHasher, GenericConfig};
use plonky2::plonk::proof::{ProofWithPublicInputs, ProofWithPublicInputsTarget};

use super::state_from_event::EventType;
use super::{hash_event, state_from_event, unbounded, unpruned};

pub struct LeafCircuit<F, C, const D: usize>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>, {
    pub event_hash: unpruned::LeafSubCircuit,
    pub partial_state: state_from_event::LeafSubCircuit,
    pub unbounded: unbounded::LeafSubCircuit,
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

        let event_hash_inputs = unpruned::SubCircuitInputs::default(&mut builder);
        let partial_state_inputs = state_from_event::SubCircuitInputs::default(&mut builder);

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

        let (circuit, unbounded) = unbounded::LeafSubCircuit::new(builder);

        let event_hash = event_hash_targets.build(&circuit.prover_only.public_inputs);
        let partial_state = partial_state_targets.build(&circuit.prover_only.public_inputs);

        Self {
            event_hash,
            partial_state,
            unbounded,
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
        self.partial_state
            .set_witness(&mut inputs, address, event_owner, event_ty, event_value);
        self.unbounded.set_witness(&mut inputs, &branch.circuit);
        self.circuit.prove(inputs)
    }
}

pub struct BranchCircuit<F, C, const D: usize>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>, {
    pub event_hash: unpruned::BranchSubCircuit,
    pub partial_state: state_from_event::BranchSubCircuit,
    pub unbounded: unbounded::BranchSubCircuit,
    pub circuit: CircuitData<F, C, D>,
    pub targets: BranchTargets<D>,
}

pub struct BranchTargets<const D: usize> {
    pub left_is_leaf: BoolTarget,
    pub right_is_leaf: BoolTarget,
    pub left_proof: ProofWithPublicInputsTarget<D>,
    pub right_proof: ProofWithPublicInputsTarget<D>,
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

        let event_hash_inputs = unpruned::SubCircuitInputs::default(&mut builder);
        let partial_state_inputs = state_from_event::SubCircuitInputs::default(&mut builder);

        let common = &leaf.circuit.common;
        let left_proof = builder.add_virtual_proof_with_pis(common);
        let right_proof = builder.add_virtual_proof_with_pis(common);
        let left_is_leaf = builder.add_virtual_bool_target_safe();
        let right_is_leaf = builder.add_virtual_bool_target_safe();
        let event_hash_targets = event_hash_inputs.from_leaf(
            &mut builder,
            &leaf.event_hash,
            &left_proof,
            &right_proof,
            false,
        );
        let partial_state_targets = partial_state_inputs.from_leaf(
            &mut builder,
            &leaf.partial_state,
            &left_proof,
            &right_proof,
        );

        let (circuit, unbounded) = unbounded::BranchSubCircuit::new(
            builder,
            &leaf.circuit,
            left_is_leaf,
            right_is_leaf,
            &left_proof,
            &right_proof,
        );
        let event_hash = event_hash_targets.from_leaf(&circuit.prover_only.public_inputs);
        let partial_state = partial_state_targets.from_leaf(&circuit.prover_only.public_inputs);
        let targets = BranchTargets {
            left_is_leaf,
            right_is_leaf,
            left_proof,
            right_proof,
        };

        Self {
            event_hash,
            partial_state,
            unbounded,
            circuit,
            targets,
        }
    }

    pub fn prove(
        &self,
        left_is_leaf: bool,
        left_proof: &ProofWithPublicInputs<F, C, D>,
        right_is_leaf: bool,
        right_proof: &ProofWithPublicInputs<F, C, D>,
    ) -> Result<ProofWithPublicInputs<F, C, D>> {
        let mut inputs = PartialWitness::new();
        self.partial_state.set_witness_from_proofs(
            &mut inputs,
            &left_proof.public_inputs,
            &right_proof.public_inputs,
        );
        inputs.set_bool_target(self.targets.left_is_leaf, left_is_leaf);
        inputs.set_bool_target(self.targets.right_is_leaf, right_is_leaf);
        inputs.set_proof_with_pis_target(&self.targets.left_proof, left_proof);
        inputs.set_proof_with_pis_target(&self.targets.right_proof, right_proof);
        self.circuit.prove(inputs)
    }
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use lazy_static::lazy_static;
    use plonky2::field::types::Field;
    use plonky2::plonk::circuit_data::CircuitConfig;

    use super::*;
    use crate::test_utils::{fast_test_circuit_config, C, D, F};

    const CONFIG: CircuitConfig = fast_test_circuit_config();

    lazy_static! {
        static ref LEAF: LeafCircuit<F, C, D> = LeafCircuit::new(&CONFIG);
        static ref BRANCH: BranchCircuit<F, C, D> = BranchCircuit::new(&CONFIG, &LEAF);
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

        let branch_proof_1 = BRANCH.prove(true, &read_proof, true, &write_proof)?;
        BRANCH.circuit.verify(branch_proof_1.clone())?;

        let branch_proof_2 = BRANCH.prove(false, &branch_proof_1, true, &ensure_proof)?;
        BRANCH.circuit.verify(branch_proof_2)?;

        Ok(())
    }
}

use anyhow::Result;
use itertools::chain;
use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::{HashOut, RichField};
use plonky2::hash::poseidon2::Poseidon2Hash;
use plonky2::iop::target::{BoolTarget, Target};
use plonky2::iop::witness::{PartialWitness, WitnessWrite};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::{CircuitConfig, CircuitData};
use plonky2::plonk::config::{AlgebraicHasher, GenericConfig};
use plonky2::plonk::proof::{ProofWithPublicInputs, ProofWithPublicInputsTarget};

use super::{byte_wise_hash_event, hash_event, propagate, unbounded, unpruned};

pub struct LeafTargets {
    /// The event type
    pub event_ty: Target,

    /// The event address
    pub event_address: Target,

    /// The event value
    pub event_value: [Target; 4],
}

pub struct LeafCircuit<F, C, const D: usize>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>, {
    /// The rc-style merkle hash of all event fields
    pub hash: unpruned::LeafSubCircuit,

    /// The vm-style merkle hash of all event fields
    pub vm_hash: unpruned::LeafSubCircuit,

    /// The owner of this event propagated throughout this tree
    pub event_owner: propagate::LeafSubCircuit<4>,

    /// The other event fields
    pub targets: LeafTargets,

    /// The recursion subcircuit
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

        let hash_inputs = unpruned::SubCircuitInputs::default(&mut builder);
        let vm_hash_inputs = unpruned::SubCircuitInputs::default(&mut builder);
        let event_owner_inputs = propagate::SubCircuitInputs::<4>::default(&mut builder);

        let hash_targets = hash_inputs.build_leaf(&mut builder);
        let vm_hash_targets = vm_hash_inputs.build_leaf(&mut builder);
        let event_owner_targets = event_owner_inputs.build(&mut builder);

        let targets = LeafTargets {
            event_ty: builder.add_virtual_target(),
            event_address: builder.add_virtual_target(),
            event_value: builder.add_virtual_target_arr::<4>(),
        };

        let event_hash = hash_event(
            &mut builder,
            event_owner_targets.inputs.values,
            targets.event_ty,
            targets.event_address,
            targets.event_value,
        );
        let event_vm_hash = byte_wise_hash_event(
            &mut builder,
            event_owner_targets.inputs.values,
            targets.event_ty,
            targets.event_address,
            targets.event_value,
        );

        builder.connect_hashes(hash_targets.inputs.unpruned_hash, event_hash);
        builder.connect_hashes(vm_hash_targets.inputs.unpruned_hash, event_vm_hash);

        let (circuit, unbounded) = unbounded::LeafSubCircuit::new(builder);

        let public_inputs = &circuit.prover_only.public_inputs;
        let hash = hash_targets.build(public_inputs);
        let vm_hash = vm_hash_targets.build(public_inputs);
        let event_owner = event_owner_targets.build_leaf(public_inputs);

        Self {
            hash,
            vm_hash,
            event_owner,
            targets,
            unbounded,
            circuit,
        }
    }

    /// `hash` only needs to be provided to check externally, otherwise it will
    /// be calculated
    pub fn prove(
        &self,
        constraint_owner: [F; 4],
        event_ty: u64,
        event_address: u64,
        event_value: [F; 4],
        hash: Option<HashOut<F>>,
        vm_hash: Option<HashOut<F>>,
        branch: &BranchCircuit<F, C, D>,
    ) -> Result<ProofWithPublicInputs<F, C, D>> {
        let mut inputs = PartialWitness::new();
        if let Some(hash) = hash {
            self.hash.set_witness(&mut inputs, hash);
        }
        if let Some(vm_hash) = vm_hash {
            self.vm_hash.set_witness(&mut inputs, vm_hash);
        }
        self.event_owner
            .set_witness(&mut inputs, constraint_owner);
        inputs.set_target(self.targets.event_ty, F::from_canonical_u64(event_ty));
        inputs.set_target(
            self.targets.event_address,
            F::from_canonical_u64(event_address),
        );
        inputs.set_target_arr(&self.targets.event_value, &event_value);
        self.unbounded.set_witness(&mut inputs, &branch.circuit);
        self.circuit.prove(inputs)
    }
}

pub struct BranchCircuit<F, C, const D: usize>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>, {
    /// The merkle hash of all events
    pub hash: unpruned::BranchSubCircuit,

    /// The vm-style merkle hash of all events
    pub vm_hash: unpruned::BranchSubCircuit,

    /// The owner of the events propagated throughout this tree
    pub event_owner: propagate::BranchSubCircuit<4>,

    pub targets: BranchTargets<D>,

    pub unbounded: unbounded::BranchSubCircuit,
    pub circuit: CircuitData<F, C, D>,
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
        let common = &leaf.circuit.common;

        let hash_inputs = unpruned::SubCircuitInputs::default(&mut builder);
        let vm_hash_inputs = unpruned::SubCircuitInputs::default(&mut builder);
        let event_owner_inputs = propagate::SubCircuitInputs::<4>::default(&mut builder);
        let left_is_leaf = builder.add_virtual_bool_target_safe();
        let right_is_leaf = builder.add_virtual_bool_target_safe();
        let left_proof = builder.add_virtual_proof_with_pis(common);
        let right_proof = builder.add_virtual_proof_with_pis(common);

        let hash_targets =
            hash_inputs.from_leaf(&mut builder, &leaf.hash, &left_proof, &right_proof, false);
        let vm_hash_targets =
            vm_hash_inputs.from_leaf(&mut builder, &leaf.hash, &left_proof, &right_proof, true);
        let event_owner_targets = event_owner_inputs.from_leaf(
            &mut builder,
            &leaf.event_owner,
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

        let hash = hash_targets.from_leaf(&circuit.prover_only.public_inputs);
        let vm_hash = vm_hash_targets.from_leaf(&circuit.prover_only.public_inputs);
        let event_owner =
            event_owner_targets.from_leaf(&circuit.prover_only.public_inputs);
        let targets = BranchTargets {
            left_is_leaf,
            right_is_leaf,
            left_proof,
            right_proof,
        };
        assert_eq!(hash.indices, leaf.hash.indices);
        assert_eq!(event_owner.indices, leaf.event_owner.indices);

        Self {
            hash,
            vm_hash,
            event_owner,
            targets,
            unbounded,
            circuit,
        }
    }

    /// `hash` and `constraint_owner` only need to be provided to check
    /// externally, otherwise they will be calculated
    pub fn prove(
        &self,
        hash: Option<HashOut<F>>,
        constraint_owner: Option<[F; 4]>,
        left_is_leaf: bool,
        right_is_leaf: bool,
        left_proof: &ProofWithPublicInputs<F, C, D>,
        right_proof: &ProofWithPublicInputs<F, C, D>,
    ) -> Result<ProofWithPublicInputs<F, C, D>> {
        let mut inputs = PartialWitness::new();
        if let Some(hash) = hash {
            self.hash.set_witness(&mut inputs, hash);
        }
        if let Some(constraint_owner) = constraint_owner {
            self.event_owner
                .set_witness(&mut inputs, constraint_owner);
        }
        inputs.set_bool_target(self.targets.left_is_leaf, left_is_leaf);
        inputs.set_bool_target(self.targets.right_is_leaf, right_is_leaf);
        inputs.set_proof_with_pis_target(&self.targets.left_proof, left_proof);
        inputs.set_proof_with_pis_target(&self.targets.right_proof, right_proof);
        self.circuit.prove(inputs)
    }
}

#[cfg(test)]
mod test {
    use std::panic::catch_unwind;

    use anyhow::Result;
    use itertools::Itertools;
    use plonky2::field::types::Field;
    use plonky2::plonk::circuit_data::CircuitConfig;
    use plonky2::plonk::config::Hasher;

    use super::*;
    use crate::test_utils::{hash_branch, C, D, F};

    fn hash_event<F: RichField>(
        constraint_owner: [F; 4],
        ty: u64,
        address: u64,
        value: [F; 4],
    ) -> HashOut<F> {
        Poseidon2Hash::hash_no_pad(
            &chain!(
                constraint_owner,
                [ty, address].map(F::from_canonical_u64),
                value,
            )
            .collect_vec(),
        )
    }
    fn byte_wise_hash_event<F: RichField>(
        constraint_owner: [F; 4],
        ty: u64,
        address: u64,
        value: [F; 4],
    ) -> HashOut<F> {
        let bytes = chain!(
                constraint_owner,
                [ty, address].map(F::from_canonical_u64),
                value,
            )
            .flat_map(|v| v.to_canonical_u64().to_le_bytes())
            .map(|v| F::from_canonical_u8(v))
            .collect_vec();
        Poseidon2Hash::hash_no_pad(&bytes)
    }

    fn hash_branch_bytes<F: RichField>(left: &HashOut<F>, right: &HashOut<F>) -> HashOut<F> {
        let bytes = chain!(left.elements, right.elements)
            .flat_map(|v| v.to_canonical_u64().to_le_bytes())
            .map(|v| F::from_canonical_u8(v))
            .collect_vec();
        Poseidon2Hash::hash_no_pad(&bytes)
    }

    #[test]
    fn verify_simple() -> Result<()> {
        let circuit_config = CircuitConfig::standard_recursion_config();
        let leaf = LeafCircuit::<F, C, D>::new(&circuit_config);
        let branch = BranchCircuit::<F, C, D>::new(&circuit_config, &leaf);
        let program_hash_1 = [4, 8, 15, 16].map(F::from_canonical_u64);

        let zero_val = [F::ZERO; 4];
        let non_zero_val_1 = [3, 1, 4, 15].map(F::from_canonical_u64);
        let non_zero_val_2 = [1, 6, 180, 33].map(F::from_canonical_u64);

        // Duplicate or conflicting events are actually fine as far as this circuit
        // cares
        let event_42_read_0 = hash_event(program_hash_1, 1, 42, zero_val);
        let event_42_write_1 = hash_event(program_hash_1, 0, 42, non_zero_val_1);
        let event_42_write_2 = hash_event(program_hash_1, 0, 42, non_zero_val_2);
        let event_bytes_42_read_0 = byte_wise_hash_event(program_hash_1, 1, 42, zero_val);
        let event_bytes_42_write_1 = byte_wise_hash_event(program_hash_1, 0, 42, non_zero_val_1);
        let event_bytes_42_write_2 = byte_wise_hash_event(program_hash_1, 0, 42, non_zero_val_2);

        // Read zero
        let read_proof = leaf.prove(
            program_hash_1,
            1,
            42,
            zero_val,
            Some(event_42_read_0),
            Some(event_bytes_42_read_0),
            &branch,
        )?;
        leaf.circuit.verify(read_proof.clone())?;

        // Write pi
        let write_proof_1 = leaf.prove(
            program_hash_1,
            0,
            42,
            non_zero_val_1,
            Some(event_42_write_1),
            Some(event_bytes_42_write_1),
            &branch,
        )?;
        leaf.circuit.verify(write_proof_1.clone())?;

        // Write phi
        let write_proof_2 = leaf.prove(
            program_hash_1,
            0,
            42,
            non_zero_val_2,
            Some(event_42_write_2),
            Some(event_bytes_42_write_2),
            &branch,
        )?;
        leaf.circuit.verify(write_proof_2.clone())?;

        let branch_1_hash = hash_branch(&event_42_write_1, &event_42_write_2);
        let branch_2_hash = hash_branch(&event_42_read_0, &branch_1_hash);
        let branch_1_bytes_hash = hash_branch_bytes(&event_42_write_1, &event_42_write_2);
        let branch_2_bytes_hash = hash_branch_bytes(&event_42_read_0, &branch_1_hash);

        // Combine writes
        let branch_proof_1 = branch.prove(
            Some(branch_1_hash),
            Some(program_hash_1),
            true,
            true,
            &write_proof_1,
            &write_proof_2,
        )?;
        branch.circuit.verify(branch_proof_1.clone())?;

        // Combine with reads
        let branch_proof_2 = branch.prove(
            Some(branch_2_hash),
            Some(program_hash_1),
            true,
            false,
            &read_proof,
            &branch_proof_1,
        )?;
        branch.circuit.verify(branch_proof_2)?;

        Ok(())
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_leaf_hash() {
        let (leaf, branch, program_hash_1, zero_val, event_42_read_1) = catch_unwind(|| {
            let circuit_config = CircuitConfig::standard_recursion_config();
            let leaf = LeafCircuit::<F, C, D>::new(&circuit_config);
            let branch = BranchCircuit::<F, C, D>::new(&circuit_config, &leaf);
            let program_hash_1 = [4, 8, 15, 16].map(F::from_canonical_u64);
            let program_hash_2 = [2, 3, 4, 2].map(F::from_canonical_u64);

            let zero_val = [F::ZERO; 4];

            let event_42_read_1 = hash_event(program_hash_2, 1, 42, zero_val);
            (leaf, branch, program_hash_1, zero_val, event_42_read_1)
        })
        .expect("shouldn't fail");

        // Fail to prove with mismatched hashes
        leaf.prove(
            program_hash_1,
            1,
            42,
            zero_val,
            Some(event_42_read_1),
            &branch,
        )
        .unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_program_match() {
        let (program_hash_1, branch, branch_1_hash, read_proof_1, read_proof_2) =
            catch_unwind(|| {
                let circuit_config = CircuitConfig::standard_recursion_config();
                let leaf = LeafCircuit::<F, C, D>::new(&circuit_config);
                let branch = BranchCircuit::<F, C, D>::new(&circuit_config, &leaf);
                let program_hash_1 = [4, 8, 15, 16].map(F::from_canonical_u64);
                let program_hash_2 = [2, 3, 4, 2].map(F::from_canonical_u64);

                let zero_val = [F::ZERO; 4];

                // Events must have the same
                let event_42_read_1 = hash_event(program_hash_1, 1, 42, zero_val);
                let event_42_read_2 = hash_event(program_hash_2, 1, 42, zero_val);

                // Read zero
                let read_proof_1 = leaf
                    .prove(
                        program_hash_1,
                        1,
                        42,
                        zero_val,
                        Some(event_42_read_1),
                        &branch,
                    )
                    .unwrap();
                leaf.circuit.verify(read_proof_1.clone()).unwrap();

                let read_proof_2 = leaf
                    .prove(
                        program_hash_2,
                        1,
                        42,
                        zero_val,
                        Some(event_42_read_2),
                        &branch,
                    )
                    .unwrap();
                leaf.circuit.verify(read_proof_2.clone()).unwrap();

                // Combine reads
                let branch_1_hash = hash_branch(&event_42_read_1, &event_42_read_2);
                (
                    program_hash_1,
                    branch,
                    branch_1_hash,
                    read_proof_1,
                    read_proof_2,
                )
            })
            .expect("shouldn't fail");

        // Fail to prove with mismatched program hashes between branches
        branch
            .prove(
                Some(branch_1_hash),
                Some(program_hash_1),
                true,
                true,
                &read_proof_1,
                &read_proof_2,
            )
            .unwrap();
    }
}

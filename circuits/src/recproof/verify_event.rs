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

use super::{propagate, unbounded, unpruned};

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
    /// The merkle hash of all event fields
    pub hash: unpruned::LeafSubCircuit,

    /// The owner of this event propagated throughout this tree
    pub constraint_owner: propagate::LeafSubCircuit<4>,

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

        let hash_inputs = unpruned::LeafInputs::default(&mut builder);
        let constraint_owner_inputs = propagate::LeafInputs::<4>::default(&mut builder);

        let hash_targets = hash_inputs.build(&mut builder);
        let constraint_owner_targets = constraint_owner_inputs.build(&mut builder);

        let targets = LeafTargets {
            event_ty: builder.add_virtual_target(),
            event_address: builder.add_virtual_target(),
            event_value: builder.add_virtual_target_arr::<4>(),
        };

        let event_hash = builder.hash_n_to_hash_no_pad::<Poseidon2Hash>(
            chain!(
                constraint_owner_targets.values,
                [targets.event_ty, targets.event_address],
                targets.event_value,
            )
            .collect(),
        );

        builder.connect_hashes(hash_targets.unpruned_hash, event_hash);

        let (circuit, unbounded) = unbounded::LeafSubCircuit::new(builder);

        let public_inputs = &circuit.prover_only.public_inputs;
        let hash = hash_targets.build(public_inputs);
        let constraint_owner = constraint_owner_targets.build(public_inputs);

        Self {
            hash,
            constraint_owner,
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
        branch: &BranchCircuit<F, C, D>,
    ) -> Result<ProofWithPublicInputs<F, C, D>> {
        let mut inputs = PartialWitness::new();
        if let Some(hash) = hash {
            self.hash.set_inputs(&mut inputs, hash);
        }
        self.constraint_owner
            .set_inputs(&mut inputs, constraint_owner);
        inputs.set_target(self.targets.event_ty, F::from_canonical_u64(event_ty));
        inputs.set_target(
            self.targets.event_address,
            F::from_canonical_u64(event_address),
        );
        inputs.set_target_arr(&self.targets.event_value, &event_value);
        self.unbounded.set_inputs(&mut inputs, &branch.circuit);
        self.circuit.prove(inputs)
    }
}

pub struct BranchCircuit<F, C, const D: usize>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>, {
    /// The merkle hash of all event fields
    pub hash: unpruned::BranchSubCircuit,

    /// The owner of the events propagated throughout this tree
    pub constraint_owner: propagate::BranchSubCircuit<4>,

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

        let hash_inputs = unpruned::BranchInputs::default(&mut builder);
        let constraint_owner_inputs = propagate::BranchInputs::<4>::default(&mut builder);
        let left_is_leaf = builder.add_virtual_bool_target_safe();
        let right_is_leaf = builder.add_virtual_bool_target_safe();
        let left_proof = builder.add_virtual_proof_with_pis(common);
        let right_proof = builder.add_virtual_proof_with_pis(common);

        let hash_targets =
            hash_inputs.from_leaf(&mut builder, &leaf.hash, &left_proof, &right_proof);
        let constraint_owner_targets = constraint_owner_inputs.from_leaf(
            &mut builder,
            &leaf.constraint_owner,
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
        let constraint_owner =
            constraint_owner_targets.from_leaf(&circuit.prover_only.public_inputs);
        let targets = BranchTargets {
            left_is_leaf,
            right_is_leaf,
            left_proof,
            right_proof,
        };
        assert_eq!(hash.indices, leaf.hash.indices);
        assert_eq!(constraint_owner.indices, leaf.constraint_owner.indices);

        Self {
            hash,
            constraint_owner,
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
            self.hash.set_inputs(&mut inputs, hash);
        }
        if let Some(constraint_owner) = constraint_owner {
            self.constraint_owner
                .set_inputs(&mut inputs, constraint_owner);
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
    // TODO
}

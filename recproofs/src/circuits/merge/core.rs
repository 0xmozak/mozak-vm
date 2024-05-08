//! Subcircuits for recursively proving the merge of two binary merkle trees.
//!
//! The resulting merge of trees A and B will provably contain all nodes from A
//! and B and those nodes will retain their original relative positioning within
//! a tree, i.e. if A1 was to the left of A2 in the original tree, it will still
//! be in the resulting tree. However no order is defined for the positioning of
//! nodes between A and B, i.e. A1 could be to the left or right of B1.

use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::{HashOut, HashOutTarget, RichField};
use plonky2::iop::target::{BoolTarget, Target};
use plonky2::iop::witness::{PartialWitness, WitnessWrite};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::proof::ProofWithPublicInputsTarget;

use crate::indices::{BoolTargetIndex, HashTargetIndex};
use crate::{at_least_one_true, hash_is_zero, hash_or_forward};

/// The indices of the public inputs of this subcircuit in any
/// `ProofWithPublicInputs`
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct PublicIndices {
    /// The index for the presence of the a hash
    pub a_present: BoolTargetIndex,

    /// The indices of each of the elements of the a hash
    pub a_hash: HashTargetIndex,

    /// The index for the presence of the b hash
    pub b_present: BoolTargetIndex,

    /// The indices of each of the elements of the b hash
    pub b_hash: HashTargetIndex,

    /// The index for the presence of the merged hash
    pub merged_present: BoolTargetIndex,

    /// The indices of each of the elements of the merged hash
    pub merged_hash: HashTargetIndex,
}

pub struct SubCircuitInputs {
    /// The presence of `a_hash`
    pub a_present: BoolTarget,

    /// The a hash or ZERO if absent
    pub a_hash: HashOutTarget,

    /// The presence of `b_hash`
    pub b_present: BoolTarget,

    /// The b hash or ZERO if absent
    pub b_hash: HashOutTarget,

    /// The presence of `merged_hash`
    pub merged_present: BoolTarget,

    /// The merged hash or ZERO if absent
    pub merged_hash: HashOutTarget,
}

pub struct LeafTargets {
    /// The public inputs
    pub inputs: SubCircuitInputs,
}

impl SubCircuitInputs {
    pub fn default<F, const D: usize>(builder: &mut CircuitBuilder<F, D>) -> Self
    where
        F: RichField + Extendable<D>, {
        let a_present = builder.add_virtual_bool_target_safe();
        let a_hash = builder.add_virtual_hash();
        let b_present = builder.add_virtual_bool_target_safe();
        let b_hash = builder.add_virtual_hash();
        let merged_present = builder.add_virtual_bool_target_safe();
        let merged_hash = builder.add_virtual_hash();
        builder.register_public_input(a_present.target);
        builder.register_public_inputs(&a_hash.elements);
        builder.register_public_input(b_present.target);
        builder.register_public_inputs(&b_hash.elements);
        builder.register_public_input(merged_present.target);
        builder.register_public_inputs(&merged_hash.elements);

        Self {
            a_present,
            a_hash,
            b_present,
            b_hash,
            merged_present,
            merged_hash,
        }
    }

    #[must_use]
    pub fn build_leaf<F, const D: usize>(self, builder: &mut CircuitBuilder<F, D>) -> LeafTargets
    where
        F: RichField + Extendable<D>, {
        let a_is_zero = hash_is_zero(builder, self.a_hash);
        let b_is_zero = hash_is_zero(builder, self.b_hash);

        at_least_one_true(builder, [a_is_zero, self.a_present]);
        at_least_one_true(builder, [b_is_zero, self.b_present]);

        let merged_present_calc = builder.or(self.a_present, self.b_present);
        builder.connect(self.merged_present.target, merged_present_calc.target);

        let merged_hash_calc = hash_or_forward(
            builder,
            self.a_present,
            self.a_hash.elements,
            self.b_present,
            self.b_hash.elements,
        );
        builder.connect_hashes(self.merged_hash, merged_hash_calc);

        LeafTargets { inputs: self }
    }
}

/// The leaf subcircuit metadata. This subcircuit merges up to two leaf hashes
/// creating a parent node if two leafs are present, otherwise just forwarding
/// any existing hash
pub struct LeafSubCircuit {
    pub targets: LeafTargets,
    pub indices: PublicIndices,
}

impl LeafTargets {
    #[must_use]
    pub fn build(self, public_inputs: &[Target]) -> LeafSubCircuit {
        // Find the indices
        let indices = PublicIndices {
            a_present: BoolTargetIndex::new(public_inputs, self.inputs.a_present),
            a_hash: HashTargetIndex::new(public_inputs, self.inputs.a_hash),
            b_present: BoolTargetIndex::new(public_inputs, self.inputs.b_present),
            b_hash: HashTargetIndex::new(public_inputs, self.inputs.b_hash),
            merged_present: BoolTargetIndex::new(public_inputs, self.inputs.merged_present),
            merged_hash: HashTargetIndex::new(public_inputs, self.inputs.merged_hash),
        };
        LeafSubCircuit {
            targets: self,
            indices,
        }
    }
}

impl LeafSubCircuit {
    pub fn set_witness<F: RichField>(
        &self,
        inputs: &mut PartialWitness<F>,
        a_hash: Option<HashOut<F>>,
        b_hash: Option<HashOut<F>>,
    ) {
        inputs.set_bool_target(self.targets.inputs.a_present, a_hash.is_some());
        inputs.set_bool_target(self.targets.inputs.b_present, b_hash.is_some());
        inputs.set_hash_target(self.targets.inputs.a_hash, a_hash.unwrap_or_default());
        inputs.set_hash_target(self.targets.inputs.b_hash, b_hash.unwrap_or_default());
    }

    #[allow(clippy::too_many_arguments)]
    pub fn set_witness_unsafe<F: RichField>(
        &self,
        inputs: &mut PartialWitness<F>,
        a_present: bool,
        a_hash: HashOut<F>,
        b_present: bool,
        b_hash: HashOut<F>,
        merged_present: bool,
        merged_hash: Option<HashOut<F>>,
    ) {
        inputs.set_bool_target(self.targets.inputs.a_present, a_present);
        inputs.set_hash_target(self.targets.inputs.a_hash, a_hash);
        inputs.set_bool_target(self.targets.inputs.b_present, b_present);
        inputs.set_hash_target(self.targets.inputs.b_hash, b_hash);
        inputs.set_bool_target(self.targets.inputs.merged_present, merged_present);
        if let Some(merged_hash) = merged_hash {
            inputs.set_hash_target(self.targets.inputs.merged_hash, merged_hash);
        }
    }
}

pub struct BranchTargets {
    /// The public inputs
    pub inputs: SubCircuitInputs,
}

impl SubCircuitInputs {
    #[must_use]
    pub fn build_branch<F, const D: usize>(
        self,
        builder: &mut CircuitBuilder<F, D>,
        indices: &PublicIndices,
        left_proof: &ProofWithPublicInputsTarget<D>,
        right_proof: &ProofWithPublicInputsTarget<D>,
    ) -> BranchTargets
    where
        F: RichField + Extendable<D>, {
        let left_a_present = indices.a_present.get(&left_proof.public_inputs);
        let left_b_present = indices.b_present.get(&left_proof.public_inputs);
        let left_merged_present = indices.merged_present.get(&left_proof.public_inputs);

        let left_a = indices.a_hash.get_any(&left_proof.public_inputs);
        let left_b = indices.b_hash.get_any(&left_proof.public_inputs);
        let left_merged = indices.merged_hash.get_any(&left_proof.public_inputs);

        let right_a_present = indices.a_present.get(&right_proof.public_inputs);
        let right_b_present = indices.b_present.get(&right_proof.public_inputs);
        let right_merged_present = indices.merged_present.get(&right_proof.public_inputs);

        let right_a = indices.a_hash.get_any(&right_proof.public_inputs);
        let right_b = indices.b_hash.get_any(&right_proof.public_inputs);
        let right_merged = indices.merged_hash.get_any(&right_proof.public_inputs);

        let a_present_calc = builder.or(left_a_present, right_a_present);
        builder.connect(self.a_present.target, a_present_calc.target);

        let b_present_calc = builder.or(left_b_present, right_b_present);
        builder.connect(self.b_present.target, b_present_calc.target);

        let merged_present_calc = builder.or(left_merged_present, right_merged_present);
        builder.connect(self.merged_present.target, merged_present_calc.target);

        let a_hash_calc =
            hash_or_forward(builder, left_a_present, left_a, right_a_present, right_a);
        let b_hash_calc =
            hash_or_forward(builder, left_b_present, left_b, right_b_present, right_b);
        let merged_hash_calc = hash_or_forward(
            builder,
            left_merged_present,
            left_merged,
            right_merged_present,
            right_merged,
        );

        builder.connect_hashes(a_hash_calc, self.a_hash);
        builder.connect_hashes(b_hash_calc, self.b_hash);
        builder.connect_hashes(merged_hash_calc, self.merged_hash);

        BranchTargets { inputs: self }
    }
}

/// The branch subcircuit metadata. This subcircuit merges up to two leaf hashes
/// creating a parent node if two leafs are present, otherwise just forwarding
/// any existing hash
pub struct BranchSubCircuit {
    pub targets: BranchTargets,
    pub indices: PublicIndices,
}

impl BranchTargets {
    #[must_use]
    pub fn build(self, child: &PublicIndices, public_inputs: &[Target]) -> BranchSubCircuit {
        // Find the indices
        let indices = PublicIndices {
            a_present: BoolTargetIndex::new(public_inputs, self.inputs.a_present),
            a_hash: HashTargetIndex::new(public_inputs, self.inputs.a_hash),
            b_present: BoolTargetIndex::new(public_inputs, self.inputs.b_present),
            b_hash: HashTargetIndex::new(public_inputs, self.inputs.b_hash),
            merged_present: BoolTargetIndex::new(public_inputs, self.inputs.merged_present),
            merged_hash: HashTargetIndex::new(public_inputs, self.inputs.merged_hash),
        };
        debug_assert_eq!(indices, *child);

        BranchSubCircuit {
            targets: self,
            indices,
        }
    }
}

impl BranchSubCircuit {
    pub fn set_witness<F: RichField>(&self, _inputs: &mut PartialWitness<F>) {}

    #[allow(clippy::too_many_arguments)]
    pub fn set_witness_unsafe<F: RichField>(
        &self,
        inputs: &mut PartialWitness<F>,
        a_present: bool,
        a_hash: Option<HashOut<F>>,
        b_present: bool,
        b_hash: Option<HashOut<F>>,
        merged_present: bool,
        merged_hash: Option<HashOut<F>>,
    ) {
        inputs.set_bool_target(self.targets.inputs.a_present, a_present);
        if let Some(a_hash) = a_hash {
            inputs.set_hash_target(self.targets.inputs.a_hash, a_hash);
        }
        inputs.set_bool_target(self.targets.inputs.b_present, b_present);
        if let Some(b_hash) = b_hash {
            inputs.set_hash_target(self.targets.inputs.b_hash, b_hash);
        }
        inputs.set_bool_target(self.targets.inputs.merged_present, merged_present);
        if let Some(merged_hash) = merged_hash {
            inputs.set_hash_target(self.targets.inputs.merged_hash, merged_hash);
        }
    }
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use plonky2::field::types::Field;
    use plonky2::plonk::circuit_data::{CircuitConfig, CircuitData};
    use plonky2::plonk::proof::ProofWithPublicInputs;

    use super::*;
    use crate::subcircuits::bounded;
    use crate::test_utils::{hash_branch, hash_str, C, CONFIG, D, F, NON_ZERO_HASHES, ZERO_HASH};

    pub struct DummyLeafCircuit {
        pub bounded: bounded::LeafSubCircuit,
        pub merge: LeafSubCircuit,
        pub circuit: CircuitData<F, C, D>,
    }

    impl DummyLeafCircuit {
        #[must_use]
        pub fn new(circuit_config: &CircuitConfig) -> Self {
            let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());

            let bounded_inputs = bounded::SubCircuitInputs::default(&mut builder);
            let merge_inputs = SubCircuitInputs::default(&mut builder);

            let bounded_targets = bounded_inputs.build_leaf(&mut builder);
            let merge_targets = merge_inputs.build_leaf(&mut builder);

            let circuit = builder.build();

            let public_inputs = &circuit.prover_only.public_inputs;
            let bounded = bounded_targets.build(public_inputs);
            let merge = merge_targets.build(public_inputs);

            Self {
                bounded,
                merge,
                circuit,
            }
        }

        pub fn prove(
            &self,
            a_tree: Option<HashOut<F>>,
            b_tree: Option<HashOut<F>>,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.bounded.set_witness(&mut inputs);
            self.merge.set_witness(&mut inputs, a_tree, b_tree);
            self.circuit.prove(inputs)
        }

        #[allow(clippy::too_many_arguments)]
        pub fn prove_unsafe(
            &self,
            a_present: bool,
            a_hash: HashOut<F>,
            b_present: bool,
            b_hash: HashOut<F>,
            merged_present: bool,
            merged_hash: Option<HashOut<F>>,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.bounded.set_witness(&mut inputs);
            self.merge.set_witness_unsafe(
                &mut inputs,
                a_present,
                a_hash,
                b_present,
                b_hash,
                merged_present,
                merged_hash,
            );
            self.circuit.prove(inputs)
        }
    }

    pub struct DummyBranchCircuit {
        pub bounded: bounded::BranchSubCircuit<D>,
        pub merge: BranchSubCircuit,
        pub circuit: CircuitData<F, C, D>,
    }

    impl DummyBranchCircuit {
        #[must_use]
        pub fn new(
            circuit_config: &CircuitConfig,
            indices: &PublicIndices,
            child: &CircuitData<F, C, D>,
        ) -> Self {
            let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());

            let bounded_inputs = bounded::SubCircuitInputs::default(&mut builder);
            let merge_inputs = SubCircuitInputs::default(&mut builder);

            let bounded_targets = bounded_inputs.build_branch(&mut builder, child);
            let merge_targets = merge_inputs.build_branch(
                &mut builder,
                indices,
                &bounded_targets.left_proof,
                &bounded_targets.right_proof,
            );

            let circuit = builder.build();

            let public_inputs = &circuit.prover_only.public_inputs;
            let bounded = bounded_targets.build(public_inputs);
            let merge = merge_targets.build(indices, public_inputs);
            Self {
                bounded,
                merge,
                circuit,
            }
        }

        #[must_use]
        pub fn from_leaf(circuit_config: &CircuitConfig, leaf: &DummyLeafCircuit) -> Self {
            Self::new(circuit_config, &leaf.merge.indices, &leaf.circuit)
        }

        #[must_use]
        pub fn from_branch(circuit_config: &CircuitConfig, branch: &Self) -> Self {
            Self::new(circuit_config, &branch.merge.indices, &branch.circuit)
        }

        pub fn prove(
            &self,
            left_proof: &ProofWithPublicInputs<F, C, D>,
            right_proof: &ProofWithPublicInputs<F, C, D>,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.bounded
                .set_witness(&mut inputs, left_proof, right_proof);
            self.merge.set_witness(&mut inputs);
            self.circuit.prove(inputs)
        }

        #[allow(clippy::too_many_arguments)]
        pub fn prove_unsafe(
            &self,
            a_present: bool,
            a_hash: Option<HashOut<F>>,
            b_present: bool,
            b_hash: Option<HashOut<F>>,
            merged_present: bool,
            merged_hash: Option<HashOut<F>>,
            left_proof: &ProofWithPublicInputs<F, C, D>,
            right_proof: &ProofWithPublicInputs<F, C, D>,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.bounded
                .set_witness(&mut inputs, left_proof, right_proof);
            self.merge.set_witness_unsafe(
                &mut inputs,
                a_present,
                a_hash,
                b_present,
                b_hash,
                merged_present,
                merged_hash,
            );
            self.circuit.prove(inputs)
        }
    }

    #[tested_fixture::tested_fixture(LEAF)]
    fn build_leaf() -> DummyLeafCircuit { DummyLeafCircuit::new(&CONFIG) }

    #[tested_fixture::tested_fixture(BRANCH_1)]
    fn build_branch_1() -> DummyBranchCircuit { DummyBranchCircuit::from_leaf(&CONFIG, &LEAF) }

    fn assert_leaf(proof: &ProofWithPublicInputs<F, C, D>, merged: Option<HashOut<F>>) {
        let indices = &LEAF.merge.indices;
        let p_present = indices.merged_present.get_any(&proof.public_inputs);
        assert_eq!(p_present, F::from_bool(merged.is_some()));

        let p_merged = indices.merged_hash.get_any(&proof.public_inputs);
        assert_eq!(p_merged, merged.unwrap_or_default().elements);
    }

    fn assert_branch(
        branch: &DummyBranchCircuit,
        proof: &ProofWithPublicInputs<F, C, D>,
        a_hash: Option<HashOut<F>>,
        b_hash: Option<HashOut<F>>,
        merged: Option<HashOut<F>>,
    ) {
        let indices = &branch.merge.indices;

        let p_a_present = indices.a_present.get_any(&proof.public_inputs);
        assert_eq!(p_a_present, F::from_bool(a_hash.is_some()));

        let p_a_hash = indices.a_hash.get_any(&proof.public_inputs);
        assert_eq!(p_a_hash, a_hash.unwrap_or_default().elements);

        let p_b_present = indices.b_present.get_any(&proof.public_inputs);
        assert_eq!(p_b_present, F::from_bool(b_hash.is_some()));

        let p_b_hash = indices.b_hash.get_any(&proof.public_inputs);
        assert_eq!(p_b_hash, b_hash.unwrap_or_default().elements);

        let p_merged_present = indices.merged_present.get_any(&proof.public_inputs);
        assert_eq!(p_merged_present, F::from_bool(merged.is_some()));

        let p_merged = indices.merged_hash.get_any(&proof.public_inputs);
        assert_eq!(p_merged, merged.unwrap_or_default().elements);
    }

    #[tested_fixture::tested_fixture(EMPTY_LEAF_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_empty_leaf() -> Result<ProofWithPublicInputs<F, C, D>> {
        let proof = LEAF.prove(None, None)?;
        assert_leaf(&proof, None);
        LEAF.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(LEFT_ZERO_LEAF_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_left_zero_leaf() -> Result<ProofWithPublicInputs<F, C, D>> {
        let proof = LEAF.prove(Some(ZERO_HASH), None)?;
        assert_leaf(&proof, Some(ZERO_HASH));
        LEAF.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(RIGHT_ZERO_LEAF_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_right_zero_leaf() -> Result<ProofWithPublicInputs<F, C, D>> {
        let proof = LEAF.prove(None, Some(ZERO_HASH))?;
        assert_leaf(&proof, Some(ZERO_HASH));
        LEAF.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(LEFT_LEAF_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_left_leaf() -> Result<ProofWithPublicInputs<F, C, D>> {
        let proof = LEAF.prove(Some(NON_ZERO_HASHES[0]), None)?;
        assert_leaf(&proof, Some(NON_ZERO_HASHES[0]));
        LEAF.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(RIGHT_LEAF_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_right_leaf() -> Result<ProofWithPublicInputs<F, C, D>> {
        let proof = LEAF.prove(None, Some(NON_ZERO_HASHES[1]))?;
        assert_leaf(&proof, Some(NON_ZERO_HASHES[1]));
        LEAF.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(BOTH_LEAF_PROOF_1: (ProofWithPublicInputs<F, C, D>, HashOut<F>))]
    fn verify_both_leaf_1() -> Result<(ProofWithPublicInputs<F, C, D>, HashOut<F>)> {
        let merged = hash_branch(&NON_ZERO_HASHES[0], &NON_ZERO_HASHES[1]);
        let proof = LEAF.prove(Some(NON_ZERO_HASHES[0]), Some(NON_ZERO_HASHES[1]))?;
        assert_leaf(&proof, Some(merged));
        LEAF.circuit.verify(proof.clone())?;
        Ok((proof, merged))
    }

    #[tested_fixture::tested_fixture(BOTH_LEAF_PROOF_2: (ProofWithPublicInputs<F, C, D>, HashOut<F>))]
    fn verify_both_leaf_2() -> Result<(ProofWithPublicInputs<F, C, D>, HashOut<F>)> {
        let merged = hash_branch(&NON_ZERO_HASHES[2], &NON_ZERO_HASHES[3]);
        let proof = LEAF.prove(Some(NON_ZERO_HASHES[2]), Some(NON_ZERO_HASHES[3]))?;
        assert_leaf(&proof, Some(merged));
        LEAF.circuit.verify(proof.clone())?;
        Ok((proof, merged))
    }

    #[test]
    fn verify_leaf() -> Result<()> {
        let a_val = hash_str("Value Alpha");
        let b_val = hash_str("Value Beta");
        let ab_hash = hash_branch(&a_val, &b_val);
        let zero_zero_hash = hash_branch(&ZERO_HASH, &ZERO_HASH);
        let a_zero_hash = hash_branch(&a_val, &ZERO_HASH);
        let zero_b_hash = hash_branch(&ZERO_HASH, &b_val);

        let proof = LEAF.prove(Some(a_val), Some(b_val))?;
        assert_leaf(&proof, Some(ab_hash));
        LEAF.circuit.verify(proof)?;

        let proof = LEAF.prove(Some(ZERO_HASH), Some(ZERO_HASH))?;
        assert_leaf(&proof, Some(zero_zero_hash));
        LEAF.circuit.verify(proof)?;

        let proof = LEAF.prove(Some(a_val), Some(ZERO_HASH))?;
        assert_leaf(&proof, Some(a_zero_hash));
        LEAF.circuit.verify(proof)?;

        let proof = LEAF.prove(Some(ZERO_HASH), Some(b_val))?;
        assert_leaf(&proof, Some(zero_b_hash));
        LEAF.circuit.verify(proof)?;

        Ok(())
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_leaf_empty_zero() {
        let proof = LEAF
            .prove_unsafe(false, ZERO_HASH, false, ZERO_HASH, true, None)
            .unwrap();
        LEAF.circuit.verify(proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_leaf_empty_left() {
        let proof = LEAF
            .prove_unsafe(false, NON_ZERO_HASHES[0], false, ZERO_HASH, true, None)
            .unwrap();
        LEAF.circuit.verify(proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_leaf_empty_right() {
        let proof = LEAF
            .prove_unsafe(false, ZERO_HASH, false, NON_ZERO_HASHES[1], true, None)
            .unwrap();
        LEAF.circuit.verify(proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_leaf_empty_both() {
        let proof = LEAF
            .prove_unsafe(
                false,
                NON_ZERO_HASHES[0],
                false,
                NON_ZERO_HASHES[1],
                true,
                None,
            )
            .unwrap();
        LEAF.circuit.verify(proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "Tried to invert zero")]
    fn bad_leaf_non_empty_left() {
        let proof = LEAF
            .prove_unsafe(false, NON_ZERO_HASHES[0], false, ZERO_HASH, false, None)
            .unwrap();
        LEAF.circuit.verify(proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "Tried to invert zero")]
    fn bad_leaf_non_empty_right() {
        let proof = LEAF
            .prove_unsafe(false, ZERO_HASH, false, NON_ZERO_HASHES[1], false, None)
            .unwrap();
        LEAF.circuit.verify(proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "Tried to invert zero")]
    fn bad_leaf_non_empty_both() {
        let proof = LEAF
            .prove_unsafe(
                false,
                NON_ZERO_HASHES[0],
                false,
                NON_ZERO_HASHES[1],
                false,
                None,
            )
            .unwrap();
        LEAF.circuit.verify(proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_leaf_forward() {
        let proof = LEAF
            .prove_unsafe(
                true,
                NON_ZERO_HASHES[0],
                false,
                ZERO_HASH,
                true,
                Some(NON_ZERO_HASHES[1]),
            )
            .unwrap();
        LEAF.circuit.verify(proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_leaf_hash() {
        let proof = LEAF
            .prove_unsafe(
                true,
                NON_ZERO_HASHES[0],
                true,
                NON_ZERO_HASHES[1],
                true,
                Some(NON_ZERO_HASHES[2]),
            )
            .unwrap();
        LEAF.circuit.verify(proof).unwrap();
    }

    #[tested_fixture::tested_fixture(EMPTY_BRANCH_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_branch_empty() -> Result<ProofWithPublicInputs<F, C, D>> {
        let proof = BRANCH_1.prove(&EMPTY_LEAF_PROOF, &EMPTY_LEAF_PROOF)?;
        assert_branch(*BRANCH_1, &proof, None, None, None);
        BRANCH_1.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[test]
    fn verify_branch_single_leafs_1() -> Result<()> {
        let merged = hash_branch(&NON_ZERO_HASHES[0], &NON_ZERO_HASHES[1]);
        let proof = BRANCH_1.prove(*LEFT_LEAF_PROOF, *RIGHT_LEAF_PROOF)?;
        assert_branch(
            *BRANCH_1,
            &proof,
            Some(NON_ZERO_HASHES[0]),
            Some(NON_ZERO_HASHES[1]),
            Some(merged),
        );
        BRANCH_1.circuit.verify(proof.clone())?;
        Ok(())
    }

    #[test]
    fn verify_branch_single_leafs_2() -> Result<()> {
        let merged = hash_branch(&NON_ZERO_HASHES[1], &NON_ZERO_HASHES[0]);
        let proof = BRANCH_1.prove(*RIGHT_LEAF_PROOF, *LEFT_LEAF_PROOF)?;
        assert_branch(
            *BRANCH_1,
            &proof,
            Some(NON_ZERO_HASHES[0]),
            Some(NON_ZERO_HASHES[1]),
            Some(merged),
        );
        BRANCH_1.circuit.verify(proof.clone())?;
        Ok(())
    }

    #[test]
    fn verify_branch_right_empty() -> Result<()> {
        // Both is (A, B) => AB
        // Empty is (∅, ∅) => ∅
        // So merged results in (A∅, B∅) => AB
        let proof = BRANCH_1.prove(&BOTH_LEAF_PROOF_1.0, *EMPTY_LEAF_PROOF)?;
        assert_branch(
            *BRANCH_1,
            &proof,
            Some(NON_ZERO_HASHES[0]),
            Some(NON_ZERO_HASHES[1]),
            Some(BOTH_LEAF_PROOF_1.1),
        );
        BRANCH_1.circuit.verify(proof)?;
        Ok(())
    }

    #[test]
    fn verify_branch_left_empty() -> Result<()> {
        // Empty is (∅, ∅) => ∅
        // Both is (A, B) => AB
        // So merged results in (∅A, ∅B) => AB
        let proof = BRANCH_1.prove(*EMPTY_LEAF_PROOF, &BOTH_LEAF_PROOF_1.0)?;
        assert_branch(
            *BRANCH_1,
            &proof,
            Some(NON_ZERO_HASHES[0]),
            Some(NON_ZERO_HASHES[1]),
            Some(BOTH_LEAF_PROOF_1.1),
        );
        BRANCH_1.circuit.verify(proof)?;
        Ok(())
    }

    #[test]
    fn verify_complex_branch_1() -> Result<()> {
        // P0 is (A, B) => AB
        // P1 is (C, D) => CD
        // So merged results should be (AC, BD) => (AB)(CD)
        let alpha = hash_branch(&NON_ZERO_HASHES[0], &NON_ZERO_HASHES[2]);
        let beta = hash_branch(&NON_ZERO_HASHES[1], &NON_ZERO_HASHES[3]);
        let merged = hash_branch(&BOTH_LEAF_PROOF_1.1, &BOTH_LEAF_PROOF_2.1);

        let mut proof = BRANCH_1.prove(&BOTH_LEAF_PROOF_1.0, &BOTH_LEAF_PROOF_2.0)?;
        assert_branch(*BRANCH_1, &proof, Some(alpha), Some(beta), Some(merged));
        BRANCH_1.circuit.verify(proof.clone())?;

        // Test that empty leafs have no effect
        let mut branch_next = DummyBranchCircuit::from_branch(&CONFIG, &BRANCH_1);
        let mut empty_proof = EMPTY_BRANCH_PROOF.clone();
        for _ in 0..4 {
            proof = branch_next.prove(&proof, &empty_proof)?;
            assert_branch(&branch_next, &proof, Some(alpha), Some(beta), Some(merged));
            branch_next.circuit.verify(proof.clone())?;

            empty_proof = branch_next.prove(&empty_proof, &empty_proof)?;
            assert_branch(&branch_next, &empty_proof, None, None, None);
            branch_next.circuit.verify(empty_proof.clone())?;

            branch_next = DummyBranchCircuit::from_branch(&CONFIG, &branch_next);
        }

        Ok(())
    }

    #[test]
    fn verify_complex_branch_2() -> Result<()> {
        // P0 is (C, D) => CD
        // P1 is (A, B) => AB
        // So merged results should be (CA, BD) => (CD)(AB)
        let alpha = hash_branch(&NON_ZERO_HASHES[2], &NON_ZERO_HASHES[0]);
        let beta = hash_branch(&NON_ZERO_HASHES[3], &NON_ZERO_HASHES[1]);
        let merged = hash_branch(&BOTH_LEAF_PROOF_2.1, &BOTH_LEAF_PROOF_1.1);

        let mut proof = BRANCH_1.prove(&BOTH_LEAF_PROOF_2.0, &BOTH_LEAF_PROOF_1.0)?;
        assert_branch(*BRANCH_1, &proof, Some(alpha), Some(beta), Some(merged));
        BRANCH_1.circuit.verify(proof.clone())?;

        // Test that empty leafs have no effect
        let mut branch_next = DummyBranchCircuit::from_branch(&CONFIG, &BRANCH_1);
        let mut empty_proof = EMPTY_BRANCH_PROOF.clone();
        for _ in 0..4 {
            proof = branch_next.prove(&proof, &empty_proof)?;
            assert_branch(&branch_next, &proof, Some(alpha), Some(beta), Some(merged));
            branch_next.circuit.verify(proof.clone())?;

            empty_proof = branch_next.prove(&empty_proof, &empty_proof)?;
            assert_branch(&branch_next, &empty_proof, None, None, None);
            branch_next.circuit.verify(empty_proof.clone())?;

            branch_next = DummyBranchCircuit::from_branch(&CONFIG, &branch_next);
        }

        Ok(())
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_branch_empty() {
        let proof = BRANCH_1
            .prove_unsafe(
                false,
                None,
                false,
                None,
                true,
                None,
                &EMPTY_LEAF_PROOF,
                &EMPTY_LEAF_PROOF,
            )
            .unwrap();
        BRANCH_1.circuit.verify(proof.clone()).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_branch_forward_left() {
        let proof = BRANCH_1
            .prove_unsafe(
                true,
                Some(NON_ZERO_HASHES[0]),
                false,
                None,
                true,
                Some(NON_ZERO_HASHES[1]),
                &LEFT_LEAF_PROOF,
                *EMPTY_LEAF_PROOF,
            )
            .unwrap();
        BRANCH_1.circuit.verify(proof.clone()).unwrap();
    }
}

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
        merged_hash: Option<HashOut<F>>,
    ) {
        self.set_witness_unsafe(
            inputs,
            a_hash.is_some(),
            a_hash.unwrap_or_default(),
            b_hash.is_some(),
            b_hash.unwrap_or_default(),
            a_hash.is_some() | b_hash.is_some(),
            merged_hash,
        );
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
        inputs.set_bool_target(self.targets.inputs.b_present, b_present);
        inputs.set_hash_target(self.targets.inputs.a_hash, a_hash);
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
    pub fn set_witness<F: RichField>(
        &self,
        inputs: &mut PartialWitness<F>,
        a_hash: Option<HashOut<F>>,
        b_hash: Option<HashOut<F>>,
        merged_hash: Option<HashOut<F>>,
    ) {
        self.set_witness_unsafe(
            inputs,
            a_hash.is_some(),
            a_hash.unwrap_or_default(),
            b_hash.is_some(),
            b_hash.unwrap_or_default(),
            a_hash.is_some() | b_hash.is_some(),
            merged_hash,
        );
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
        inputs.set_bool_target(self.targets.inputs.b_present, b_present);
        inputs.set_hash_target(self.targets.inputs.a_hash, a_hash);
        inputs.set_hash_target(self.targets.inputs.b_hash, b_hash);
        inputs.set_bool_target(self.targets.inputs.merged_present, merged_present);
        if let Some(merged_hash) = merged_hash {
            inputs.set_hash_target(self.targets.inputs.merged_hash, merged_hash);
        }
    }
}

#[cfg(test)]
mod test {
    use std::panic::catch_unwind;

    use anyhow::Result;
    use lazy_static::lazy_static;
    use plonky2::field::types::Field;
    use plonky2::hash::hash_types::NUM_HASH_OUT_ELTS;
    use plonky2::plonk::circuit_data::{CircuitConfig, CircuitData};
    use plonky2::plonk::proof::ProofWithPublicInputs;

    use super::*;
    use crate::subcircuits::bounded;
    use crate::test_utils::{hash_branch, hash_str, C, CONFIG, D, F};

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
            merged_hash: Option<HashOut<F>>,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.bounded.set_witness(&mut inputs);
            self.merge
                .set_witness(&mut inputs, a_tree, b_tree, merged_hash);
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
            a_tree: Option<HashOut<F>>,
            b_tree: Option<HashOut<F>>,
            merged_hash: Option<HashOut<F>>,
            left_proof: &ProofWithPublicInputs<F, C, D>,
            right_proof: &ProofWithPublicInputs<F, C, D>,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.bounded
                .set_witness(&mut inputs, left_proof, right_proof);
            self.merge
                .set_witness(&mut inputs, a_tree, b_tree, merged_hash);
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

    lazy_static! {
        static ref LEAF: DummyLeafCircuit = DummyLeafCircuit::new(&CONFIG);
        static ref BRANCH_1: DummyBranchCircuit = DummyBranchCircuit::from_leaf(&CONFIG, &LEAF);
    }

    #[test]
    fn verify_leaf() -> Result<()> {
        let zero_hash = HashOut::from([F::ZERO; NUM_HASH_OUT_ELTS]);
        let a_val = hash_str("Value Alpha");
        let b_val = hash_str("Value Beta");
        let ab_hash = hash_branch(&a_val, &b_val);
        let zero_zero_hash = hash_branch(&zero_hash, &zero_hash);
        let a_zero_hash = hash_branch(&a_val, &zero_hash);
        let zero_b_hash = hash_branch(&zero_hash, &b_val);

        let proof = LEAF.prove(None, None, Some(zero_hash))?;
        LEAF.circuit.verify(proof)?;

        let proof = LEAF.prove(Some(a_val), None, Some(a_val))?;
        LEAF.circuit.verify(proof)?;

        let proof = LEAF.prove(None, Some(b_val), Some(b_val))?;
        LEAF.circuit.verify(proof)?;

        let proof = LEAF.prove(Some(a_val), Some(b_val), Some(ab_hash))?;
        LEAF.circuit.verify(proof)?;

        let proof = LEAF.prove(Some(zero_hash), None, Some(zero_hash))?;
        LEAF.circuit.verify(proof)?;

        let proof = LEAF.prove(None, Some(zero_hash), Some(zero_hash))?;
        LEAF.circuit.verify(proof)?;

        let proof = LEAF.prove(Some(zero_hash), Some(zero_hash), Some(zero_zero_hash))?;
        LEAF.circuit.verify(proof)?;

        let proof = LEAF.prove(Some(a_val), Some(zero_hash), Some(a_zero_hash))?;
        LEAF.circuit.verify(proof)?;

        let proof = LEAF.prove(Some(zero_hash), Some(b_val), Some(zero_b_hash))?;
        LEAF.circuit.verify(proof)?;

        Ok(())
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_leaf_empty() {
        let zero_hash = HashOut::from([F::ZERO; NUM_HASH_OUT_ELTS]);

        let proof = LEAF
            .prove_unsafe(false, zero_hash, false, zero_hash, true, None)
            .unwrap();
        LEAF.circuit.verify(proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_leaf_forward() {
        let non_zero_hash_1 = hash_str("Non-Zero Hash 1");
        let non_zero_hash_2 = hash_str("Non-Zero Hash 2");

        let proof = LEAF
            .prove(Some(non_zero_hash_1), None, Some(non_zero_hash_2))
            .unwrap();
        LEAF.circuit.verify(proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_leaf_hash() {
        let non_zero_hash_1 = hash_str("Non-Zero Hash 1");
        let non_zero_hash_2 = hash_str("Non-Zero Hash 2");
        let non_zero_hash_3 = hash_str("Non-Zero Hash 3");

        let proof = LEAF
            .prove(
                Some(non_zero_hash_1),
                Some(non_zero_hash_2),
                Some(non_zero_hash_3),
            )
            .unwrap();
        LEAF.circuit.verify(proof).unwrap();
    }

    #[test]
    fn verify_branch_empty() -> Result<()> {
        let zero_hash = HashOut::from([F::ZERO; NUM_HASH_OUT_ELTS]);

        let empty_proof = LEAF.prove(None, None, Some(zero_hash))?;
        LEAF.circuit.verify(empty_proof.clone())?;

        let proof = BRANCH_1.prove(None, None, Some(zero_hash), &empty_proof, &empty_proof)?;
        BRANCH_1.circuit.verify(proof.clone())?;

        Ok(())
    }

    #[test]
    fn verify_branch_single() -> Result<()> {
        let a_val = hash_str("Value Alpha");
        let b_val = hash_str("Value Beta");
        let ab_hash = hash_branch(&a_val, &b_val);

        let a_proof = LEAF.prove(Some(a_val), None, Some(a_val))?;
        LEAF.circuit.verify(a_proof.clone())?;

        let b_proof = LEAF.prove(None, Some(b_val), Some(b_val))?;
        LEAF.circuit.verify(b_proof.clone())?;

        let proof = BRANCH_1.prove(Some(a_val), Some(b_val), Some(ab_hash), &a_proof, &b_proof)?;
        BRANCH_1.circuit.verify(proof.clone())?;

        Ok(())
    }

    #[test]
    fn verify_branch_multi() -> Result<()> {
        let zero_hash = HashOut::from([F::ZERO; NUM_HASH_OUT_ELTS]);
        let a_val = hash_str("Value Alpha");
        let b_val = hash_str("Value Beta");
        let ab_hash = hash_branch(&a_val, &b_val);

        let empty_proof = LEAF.prove(None, None, Some(zero_hash))?;
        LEAF.circuit.verify(empty_proof.clone())?;

        let ab_proof = LEAF.prove(Some(a_val), Some(b_val), Some(ab_hash))?;
        LEAF.circuit.verify(ab_proof.clone())?;

        let proof = BRANCH_1.prove(
            Some(a_val),
            Some(b_val),
            Some(ab_hash),
            &ab_proof,
            &empty_proof,
        )?;
        BRANCH_1.circuit.verify(proof)?;

        Ok(())
    }

    #[test]
    #[allow(clippy::similar_names)]
    fn verify_complex_branch() -> Result<()> {
        let zero_hash = HashOut::from([F::ZERO; NUM_HASH_OUT_ELTS]);
        let a_val = hash_str("Value Alpha");
        let b_val = hash_str("Value Beta");
        let c_val = hash_str("Value Gamma");
        let d_val = hash_str("Value Delta");
        let ab_hash = hash_branch(&a_val, &b_val);
        let ac_hash = hash_branch(&a_val, &c_val);
        let bd_hash = hash_branch(&b_val, &d_val);
        let cd_hash = hash_branch(&c_val, &d_val);
        let abcd_hash = hash_branch(&ab_hash, &cd_hash);

        // Imagine we want to merge tree AC with tree BD as follows:
        //    ABCD
        //  /\    /\
        // A  B  C  D

        let empty_proof = LEAF.prove(None, None, Some(zero_hash))?;
        LEAF.circuit.verify(empty_proof.clone())?;

        let ab_proof = LEAF.prove(Some(a_val), Some(b_val), Some(ab_hash))?;
        LEAF.circuit.verify(ab_proof.clone())?;

        let cd_proof = LEAF.prove(Some(c_val), Some(d_val), Some(cd_hash))?;
        LEAF.circuit.verify(cd_proof.clone())?;

        let mut empty_proof =
            BRANCH_1.prove(None, None, Some(zero_hash), &empty_proof, &empty_proof)?;
        BRANCH_1.circuit.verify(empty_proof.clone())?;

        let mut abcd_proof = BRANCH_1.prove(
            Some(ac_hash),
            Some(bd_hash),
            Some(abcd_hash),
            &ab_proof,
            &cd_proof,
        )?;
        BRANCH_1.circuit.verify(abcd_proof.clone())?;

        let mut branch_2 = DummyBranchCircuit::from_branch(&CONFIG, &BRANCH_1);

        // Test that empty leafs have no effect
        for _ in 0..4 {
            abcd_proof = branch_2.prove(
                Some(ac_hash),
                Some(bd_hash),
                Some(abcd_hash),
                &abcd_proof,
                &empty_proof,
            )?;
            branch_2.circuit.verify(abcd_proof.clone())?;

            empty_proof =
                branch_2.prove(None, None, Some(zero_hash), &empty_proof, &empty_proof)?;
            branch_2.circuit.verify(empty_proof.clone())?;

            branch_2 = DummyBranchCircuit::from_branch(&CONFIG, &branch_2);
        }

        Ok(())
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_branch_empty() {
        let (zero_hash, empty_proof) = catch_unwind(|| {
            let zero_hash = HashOut::from([F::ZERO; NUM_HASH_OUT_ELTS]);

            let empty_proof = LEAF.prove(None, None, Some(zero_hash)).unwrap();
            LEAF.circuit.verify(empty_proof.clone()).unwrap();

            (zero_hash, empty_proof)
        })
        .expect("shouldn't fail");

        let proof = BRANCH_1
            .prove_unsafe(
                false,
                zero_hash,
                false,
                zero_hash,
                true,
                Some(zero_hash),
                &empty_proof,
                &empty_proof,
            )
            .unwrap();
        BRANCH_1.circuit.verify(proof.clone()).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_branch_forward() {
        let (non_zero_hash_1, non_zero_hash_2, empty_proof, a_proof) = catch_unwind(|| {
            let zero_hash = HashOut::from([F::ZERO; NUM_HASH_OUT_ELTS]);
            let non_zero_hash_1 = hash_str("Non-Zero Hash 1");
            let non_zero_hash_2 = hash_str("Non-Zero Hash 2");

            let empty_proof = LEAF.prove(None, None, Some(zero_hash)).unwrap();
            LEAF.circuit.verify(empty_proof.clone()).unwrap();

            let a_proof = LEAF
                .prove(Some(non_zero_hash_1), None, Some(non_zero_hash_1))
                .unwrap();
            LEAF.circuit.verify(a_proof.clone()).unwrap();

            (non_zero_hash_1, non_zero_hash_2, empty_proof, a_proof)
        })
        .expect("shouldn't fail");

        let proof = BRANCH_1
            .prove(
                Some(non_zero_hash_1),
                None,
                Some(non_zero_hash_2),
                &a_proof,
                &empty_proof,
            )
            .unwrap();
        BRANCH_1.circuit.verify(proof.clone()).unwrap();
    }
}

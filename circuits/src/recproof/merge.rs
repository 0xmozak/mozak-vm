//! Subcircuits for recursively proving the merge of two binary merkle trees.
//!
//! The resulting merge of trees A and B will provably contain all nodes from A
//! and B and those nodes will retain their original relative positioning within
//! a tree, i.e. if A1 was to the left of A2 in the original tree, it will still
//! be in the resulting tree. However no order is defined for the positioning of
//! nodes between A and B, i.e. A1 could be to the left or right of B1.

use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::{HashOut, HashOutTarget, RichField, NUM_HASH_OUT_ELTS};
use plonky2::iop::target::{BoolTarget, Target};
use plonky2::iop::witness::{PartialWitness, WitnessWrite};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::proof::ProofWithPublicInputsTarget;

use super::{find_hash, hash_or_forward_zero, hashes_equal};

/// The indices of the public inputs of this subcircuit in any
/// `ProofWithPublicInputs`
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct PublicIndices {
    /// The indices of each of the elements of the a hash
    pub a_hash: [usize; NUM_HASH_OUT_ELTS],
    /// The indices of each of the elements of the b hash
    pub b_hash: [usize; NUM_HASH_OUT_ELTS],
    /// The indices of each of the elements of the merged hash
    pub merged_hash: [usize; NUM_HASH_OUT_ELTS],
}

impl PublicIndices {
    /// Extract `a_hash` from an array of public inputs.
    pub fn get_a_hash<T: Copy>(&self, public_inputs: &[T]) -> [T; NUM_HASH_OUT_ELTS] {
        self.a_hash.map(|i| public_inputs[i])
    }

    /// Insert `a_hash` into an array of public inputs.
    pub fn set_a_hash<T>(&self, public_inputs: &mut [T], v: [T; NUM_HASH_OUT_ELTS]) {
        for (i, v) in v.into_iter().enumerate() {
            public_inputs[self.a_hash[i]] = v;
        }
    }

    /// Extract `b_hash` from an array of public inputs.
    pub fn get_b_hash<T: Copy>(&self, public_inputs: &[T]) -> [T; NUM_HASH_OUT_ELTS] {
        self.b_hash.map(|i| public_inputs[i])
    }

    /// Insert `a_hash` into an array of public inputs.
    pub fn set_b_hash<T>(&self, public_inputs: &mut [T], v: [T; NUM_HASH_OUT_ELTS]) {
        for (i, v) in v.into_iter().enumerate() {
            public_inputs[self.b_hash[i]] = v;
        }
    }

    /// Extract `merged_hash` from an array of public inputs.
    pub fn get_merged_hash<T: Copy>(&self, public_inputs: &[T]) -> [T; NUM_HASH_OUT_ELTS] {
        self.merged_hash.map(|i| public_inputs[i])
    }

    /// Insert `a_hash` into an array of public inputs.
    pub fn set_merged_hash<T>(&self, public_inputs: &mut [T], v: [T; NUM_HASH_OUT_ELTS]) {
        for (i, v) in v.into_iter().enumerate() {
            public_inputs[self.merged_hash[i]] = v;
        }
    }
}

pub struct SubCircuitInputs {
    /// The a hash
    pub a_hash: HashOutTarget,

    /// The b hash
    pub b_hash: HashOutTarget,

    /// The merged hash
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
        let a_hash = builder.add_virtual_hash();
        let b_hash = builder.add_virtual_hash();
        let merged_hash = builder.add_virtual_hash();
        builder.register_public_inputs(&a_hash.elements);
        builder.register_public_inputs(&b_hash.elements);
        builder.register_public_inputs(&merged_hash.elements);

        Self {
            a_hash,
            b_hash,
            merged_hash,
        }
    }

    #[must_use]
    pub fn build_leaf<F, const D: usize>(self, builder: &mut CircuitBuilder<F, D>) -> LeafTargets
    where
        F: RichField + Extendable<D>, {
        let merged_hash_calc =
            hash_or_forward_zero(builder, self.a_hash.elements, self.b_hash.elements);
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
        // Find the indicies
        let indices = PublicIndices {
            a_hash: find_hash(public_inputs, self.inputs.a_hash),
            b_hash: find_hash(public_inputs, self.inputs.b_hash),
            merged_hash: find_hash(public_inputs, self.inputs.merged_hash),
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
        a_hash: HashOut<F>,
        b_hash: HashOut<F>,
        merged_hash: Option<HashOut<F>>,
    ) {
        inputs.set_hash_target(self.targets.inputs.a_hash, a_hash);
        inputs.set_hash_target(self.targets.inputs.b_hash, b_hash);
        if let Some(merged_hash) = merged_hash {
            inputs.set_hash_target(self.targets.inputs.merged_hash, merged_hash);
        }
    }
}

pub struct BranchTargets {
    /// The public inputs
    pub inputs: SubCircuitInputs,

    /// Indicates if the left branch is a leaf or not
    pub left_is_leaf: BoolTarget,

    /// Indicates if the right branch is a leaf or not
    pub right_is_leaf: BoolTarget,
}

impl SubCircuitInputs {
    #[must_use]
    pub fn build<F, const D: usize>(
        self,
        builder: &mut CircuitBuilder<F, D>,
        indices: &PublicIndices,
        left_proof: &ProofWithPublicInputsTarget<D>,
        right_proof: &ProofWithPublicInputsTarget<D>,
    ) -> BranchTargets
    where
        F: RichField + Extendable<D>, {
        let left_a = indices.get_a_hash(&left_proof.public_inputs);
        let left_b = indices.get_b_hash(&left_proof.public_inputs);
        let left_merged = indices.get_merged_hash(&left_proof.public_inputs);
        let left_is_leaf = {
            let left_ab_hash = hash_or_forward_zero(builder, left_a, left_b);
            hashes_equal(builder, left_merged.into(), left_ab_hash)
        };

        let right_a = indices.get_a_hash(&right_proof.public_inputs);
        let right_b = indices.get_b_hash(&right_proof.public_inputs);
        let right_merged = indices.get_merged_hash(&right_proof.public_inputs);
        let right_is_leaf = {
            let right_ab_hash = hash_or_forward_zero(builder, right_a, right_b);
            hashes_equal(builder, right_merged.into(), right_ab_hash)
        };

        let a_hash_calc = hash_or_forward_zero(builder, left_a, right_a);
        let b_hash_calc = hash_or_forward_zero(builder, left_b, right_b);
        let merged_hash_calc = hash_or_forward_zero(builder, left_merged, right_merged);

        builder.connect_hashes(a_hash_calc, self.a_hash);
        builder.connect_hashes(b_hash_calc, self.b_hash);
        builder.connect_hashes(merged_hash_calc, self.merged_hash);

        BranchTargets {
            inputs: self,
            left_is_leaf,
            right_is_leaf,
        }
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
        // Find the indicies
        let indices = PublicIndices {
            a_hash: find_hash(public_inputs, self.inputs.a_hash),
            b_hash: find_hash(public_inputs, self.inputs.b_hash),
            merged_hash: find_hash(public_inputs, self.inputs.merged_hash),
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
        a_hash: HashOut<F>,
        b_hash: HashOut<F>,
        merged_hash: Option<HashOut<F>>,
    ) {
        inputs.set_hash_target(self.targets.inputs.a_hash, a_hash);
        inputs.set_hash_target(self.targets.inputs.b_hash, b_hash);
        if let Some(merged_hash) = merged_hash {
            inputs.set_hash_target(self.targets.inputs.merged_hash, merged_hash);
        }
    }
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use lazy_static::lazy_static;
    use plonky2::field::types::Field;
    use plonky2::plonk::circuit_data::{CircuitConfig, CircuitData};
    use plonky2::plonk::proof::ProofWithPublicInputs;

    use super::*;
    use crate::recproof::bounded;
    use crate::test_utils::{fast_test_circuit_config, hash_branch, hash_str, C, D, F};

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
            a_tree: HashOut<F>,
            b_tree: HashOut<F>,
            merged_hash: Option<HashOut<F>>,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.bounded.set_witness(&mut inputs);
            self.merge
                .set_witness(&mut inputs, a_tree, b_tree, merged_hash);
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
            indicies: &PublicIndices,
            child: &CircuitData<F, C, D>,
        ) -> Self {
            let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());

            let bounded_inputs = bounded::SubCircuitInputs::default(&mut builder);
            let merge_inputs = SubCircuitInputs::default(&mut builder);

            let bounded_targets = bounded_inputs.build_branch(&mut builder, child);
            let merge_targets = merge_inputs.build(
                &mut builder,
                indicies,
                &bounded_targets.left_proof,
                &bounded_targets.right_proof,
            );

            let circuit = builder.build();

            let public_inputs = &circuit.prover_only.public_inputs;
            let bounded = bounded_targets.build(public_inputs);
            let merge = merge_targets.build(indicies, public_inputs);
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
            a_tree: HashOut<F>,
            b_tree: HashOut<F>,
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
    }

    const CONFIG: CircuitConfig = fast_test_circuit_config();

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

        let proof = LEAF.prove(zero_hash, zero_hash, Some(zero_hash))?;
        LEAF.circuit.verify(proof)?;

        let proof = LEAF.prove(a_val, zero_hash, Some(a_val))?;
        LEAF.circuit.verify(proof)?;

        let proof = LEAF.prove(zero_hash, b_val, Some(b_val))?;
        LEAF.circuit.verify(proof)?;

        let proof = LEAF.prove(a_val, b_val, Some(ab_hash))?;
        LEAF.circuit.verify(proof)?;

        Ok(())
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_zero_leaf() {
        let zero_hash = HashOut::from([F::ZERO; NUM_HASH_OUT_ELTS]);
        let non_zero_hash = hash_str("Non-Zero Hash");

        let proof = LEAF
            .prove(zero_hash, zero_hash, Some(non_zero_hash))
            .unwrap();
        LEAF.circuit.verify(proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_leaf_mismatch() {
        let zero_hash = HashOut::from([F::ZERO; NUM_HASH_OUT_ELTS]);
        let non_zero_hash_1 = hash_str("Non-Zero Hash 1");
        let non_zero_hash_2 = hash_str("Non-Zero Hash 2");

        let proof = LEAF
            .prove(non_zero_hash_1, zero_hash, Some(non_zero_hash_2))
            .unwrap();
        LEAF.circuit.verify(proof).unwrap();
    }

    #[test]
    #[allow(clippy::similar_names)]
    fn verify_branch() -> Result<()> {
        let zero_hash = HashOut::from([F::ZERO; NUM_HASH_OUT_ELTS]);
        let a_val = hash_str("Value Alpha");
        let b_val = hash_str("Value Beta");
        let ab_hash = hash_branch(&a_val, &b_val);

        let a_proof = LEAF.prove(a_val, zero_hash, Some(a_val))?;
        LEAF.circuit.verify(a_proof.clone())?;

        let b_proof = LEAF.prove(zero_hash, b_val, Some(b_val))?;
        LEAF.circuit.verify(b_proof.clone())?;

        // In practice, you should never merge two single value leafs because doing so
        // results in a terminal proof which can't be recursed
        let proof = BRANCH_1.prove(a_val, b_val, Some(ab_hash), &a_proof, &b_proof)?;
        BRANCH_1.circuit.verify(proof.clone())?;

        // Test that multi-value leafs work
        let empty_proof = LEAF.prove(zero_hash, zero_hash, Some(zero_hash))?;
        LEAF.circuit.verify(empty_proof.clone())?;

        let ab_proof = LEAF.prove(a_val, b_val, Some(ab_hash))?;
        LEAF.circuit.verify(ab_proof.clone())?;

        let proof = BRANCH_1.prove(a_val, b_val, Some(ab_hash), &ab_proof, &empty_proof)?;
        BRANCH_1.circuit.verify(proof)?;

        Ok(())
    }

    #[test]
    #[allow(clippy::similar_names)]
    fn verify_branch2() -> Result<()> {
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

        let empty_proof = LEAF.prove(zero_hash, zero_hash, Some(zero_hash))?;
        LEAF.circuit.verify(empty_proof.clone())?;

        let ab_proof = LEAF.prove(a_val, b_val, Some(ab_hash))?;
        LEAF.circuit.verify(ab_proof.clone())?;

        let cd_proof = LEAF.prove(c_val, d_val, Some(cd_hash))?;
        LEAF.circuit.verify(cd_proof.clone())?;

        let mut empty_proof = BRANCH_1.prove(
            zero_hash,
            zero_hash,
            Some(zero_hash),
            &empty_proof,
            &empty_proof,
        )?;
        BRANCH_1.circuit.verify(empty_proof.clone())?;

        let mut abcd_proof =
            BRANCH_1.prove(ac_hash, bd_hash, Some(abcd_hash), &ab_proof, &cd_proof)?;
        BRANCH_1.circuit.verify(abcd_proof.clone())?;

        let mut branch_2 = DummyBranchCircuit::from_branch(&CONFIG, &BRANCH_1);

        // Test that empty leafs have no effect
        for _ in 0..4 {
            abcd_proof =
                branch_2.prove(ac_hash, bd_hash, Some(abcd_hash), &abcd_proof, &empty_proof)?;
            branch_2.circuit.verify(abcd_proof.clone())?;

            empty_proof = branch_2.prove(
                zero_hash,
                zero_hash,
                Some(zero_hash),
                &empty_proof,
                &empty_proof,
            )?;
            branch_2.circuit.verify(empty_proof.clone())?;

            branch_2 = DummyBranchCircuit::from_branch(&CONFIG, &branch_2);
        }

        Ok(())
    }
}

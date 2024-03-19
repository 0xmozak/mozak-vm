//! Subcircuits for recursively proving the construction of a binary merkle tree
//! out of a single value.
//!
//! These subcircuits are recursive, building on top of each other to
//! create the next level up of the merkle tree.
use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::{HashOut, HashOutTarget, RichField, NUM_HASH_OUT_ELTS};
use plonky2::iop::target::{BoolTarget, Target};
use plonky2::iop::witness::{PartialWitness, WitnessWrite};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::proof::ProofWithPublicInputsTarget;

use super::{hash_is_nonzero, hash_is_zero, hash_or_forward, hashes_equal};
use crate::recproof::find_hash;

/// The indices of the public inputs of this subcircuit in any
/// `ProofWithPublicInputs`
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct PublicIndices {
    /// The indices of each of the elements of the hash
    pub hash: [usize; NUM_HASH_OUT_ELTS],

    /// The indices of each of the elements of the leaf_value
    pub leaf_value: [usize; NUM_HASH_OUT_ELTS],
}

impl PublicIndices {
    /// Extract `hash` from an array of public inputs.
    pub fn get_hash<T: Copy>(&self, public_inputs: &[T]) -> [T; NUM_HASH_OUT_ELTS] {
        self.hash.map(|i| public_inputs[i])
    }

    /// Insert `hash` into an array of public inputs.
    pub fn set_hash<T>(&self, public_inputs: &mut [T], v: [T; NUM_HASH_OUT_ELTS]) {
        for (i, v) in v.into_iter().enumerate() {
            public_inputs[self.hash[i]] = v;
        }
    }

    /// Extract `leaf_value` from an array of public inputs.
    pub fn get_leaf_value<T: Copy>(&self, public_inputs: &[T]) -> [T; NUM_HASH_OUT_ELTS] {
        self.leaf_value.map(|i| public_inputs[i])
    }

    /// Insert `leaf_value` into an array of public inputs.
    pub fn set_leaf_value<T>(&self, public_inputs: &mut [T], v: [T; NUM_HASH_OUT_ELTS]) {
        for (i, v) in v.into_iter().enumerate() {
            public_inputs[self.leaf_value[i]] = v;
        }
    }
}

pub struct SubCircuitInputs {
    /// The leaf value or ZERO if absent
    pub hash: HashOutTarget,

    /// The value to be propagated throughout the produced tree
    pub leaf_value: HashOutTarget,
}

pub struct LeafTargets {
    /// The public inputs
    pub inputs: SubCircuitInputs,
}

impl SubCircuitInputs {
    pub fn default<F, const D: usize>(builder: &mut CircuitBuilder<F, D>) -> Self
    where
        F: RichField + Extendable<D>, {
        let hash = builder.add_virtual_hash();
        let leaf_value = builder.add_virtual_hash();
        builder.register_public_inputs(&hash.elements);
        builder.register_public_inputs(&leaf_value.elements);
        Self { hash, leaf_value }
    }

    #[must_use]
    pub fn build_leaf<F, const D: usize>(self, builder: &mut CircuitBuilder<F, D>) -> LeafTargets
    where
        F: RichField + Extendable<D>, {
        let one = builder.one();

        let eq = hashes_equal(builder, self.hash, self.leaf_value);
        let zero = hash_is_zero(builder, self.hash);
        let xor = builder.add(eq.target, zero.target);
        builder.connect(xor, one);

        LeafTargets { inputs: self }
    }
}

/// The leaf subcircuit metadata. This subcircuit does basically nothing, simply
/// expressing that a hash exists
pub struct LeafSubCircuit {
    pub targets: LeafTargets,
    pub indices: PublicIndices,
}

impl LeafTargets {
    #[must_use]
    pub fn build(self, public_inputs: &[Target]) -> LeafSubCircuit {
        // Find the indicies
        let indices = PublicIndices {
            hash: find_hash(public_inputs, self.inputs.hash),
            leaf_value: find_hash(public_inputs, self.inputs.leaf_value),
        };
        LeafSubCircuit {
            targets: self,
            indices,
        }
    }
}

impl LeafSubCircuit {
    /// Get ready to generate a proof
    pub fn set_witness<F: RichField>(
        &self,
        inputs: &mut PartialWitness<F>,
        present: bool,
        leaf_value: HashOut<F>,
    ) {
        let hash = if present {
            leaf_value
        } else {
            HashOut::default()
        };
        self.set_witness_unsafe(inputs, hash, leaf_value);
    }

    pub fn set_witness_unsafe<F: RichField>(
        &self,
        inputs: &mut PartialWitness<F>,
        hash: HashOut<F>,
        leaf_value: HashOut<F>,
    ) {
        inputs.set_hash_target(self.targets.inputs.hash, hash);
        inputs.set_hash_target(self.targets.inputs.leaf_value, leaf_value);
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
    pub fn build_branch<F, const D: usize>(
        self,
        builder: &mut CircuitBuilder<F, D>,
        leaf: &LeafSubCircuit,
        left_proof: &ProofWithPublicInputsTarget<D>,
        right_proof: &ProofWithPublicInputsTarget<D>,
    ) -> BranchTargets
    where
        F: RichField + Extendable<D>, {
        let l_hash = leaf.indices.get_hash(&left_proof.public_inputs);
        let r_hash = leaf.indices.get_hash(&right_proof.public_inputs);

        // Get presence
        let left_non_zero = hash_is_nonzero(builder, l_hash);
        let right_non_zero = hash_is_nonzero(builder, r_hash);

        // Select the hash based on presence
        let summary_hash = hash_or_forward(builder, left_non_zero, l_hash, right_non_zero, r_hash);
        builder.connect_hashes(self.hash, summary_hash);

        // Make sure the leaf values are the same
        let l_leaf = leaf.indices.get_leaf_value(&left_proof.public_inputs);
        let r_leaf = leaf.indices.get_leaf_value(&right_proof.public_inputs);
        let l_leaf = HashOutTarget::from(l_leaf);
        let r_leaf = HashOutTarget::from(r_leaf);
        builder.connect_hashes(self.leaf_value, l_leaf);
        builder.connect_hashes(self.leaf_value, r_leaf);

        // Determine the type of the proof by looking at its public input
        // This works because only the root is allowed to be a incomplete (one-sided)
        // branch
        let l_hash = HashOutTarget::from(l_hash);
        let left_eq = hashes_equal(builder, l_hash, self.leaf_value);
        let left_zero = hash_is_zero(builder, l_hash);
        let left_is_leaf = builder.add(left_eq.target, left_zero.target);
        let left_is_leaf = BoolTarget::new_unsafe(left_is_leaf);
        builder.assert_bool(left_is_leaf);

        let r_hash = HashOutTarget::from(r_hash);
        let right_eq = hashes_equal(builder, r_hash, self.leaf_value);
        let right_zero = hash_is_zero(builder, r_hash);
        let right_is_leaf = builder.add(right_eq.target, right_zero.target);
        let right_is_leaf = BoolTarget::new_unsafe(right_is_leaf);
        builder.assert_bool(right_is_leaf);

        BranchTargets {
            inputs: self,
            left_is_leaf,
            right_is_leaf,
        }
    }
}

pub struct BranchSubCircuit {
    pub targets: BranchTargets,
    pub indices: PublicIndices,
}

impl BranchTargets {
    #[must_use]
    pub fn build(self, leaf: &LeafSubCircuit, public_inputs: &[Target]) -> BranchSubCircuit {
        // Find the indicies
        let indices = PublicIndices {
            hash: find_hash(public_inputs, self.inputs.hash),
            leaf_value: find_hash(public_inputs, self.inputs.leaf_value),
        };
        debug_assert_eq!(leaf.indices, indices);

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
        hash: HashOut<F>,
        leaf_value: HashOut<F>,
    ) {
        inputs.set_hash_target(self.targets.inputs.hash, hash);
        inputs.set_hash_target(self.targets.inputs.leaf_value, leaf_value);
    }
}
#[cfg(test)]
mod test {
    use anyhow::Result;
    use plonky2::field::types::Field;
    use plonky2::hash::hash_types::{HashOut, NUM_HASH_OUT_ELTS};
    use plonky2::plonk::circuit_data::{CircuitConfig, CircuitData};
    use plonky2::plonk::proof::ProofWithPublicInputs;

    use super::*;
    use crate::recproof::unbounded;
    use crate::test_utils::{hash_branch, hash_str, C, D, F};

    pub struct DummyLeafCircuit {
        pub make_tree: LeafSubCircuit,
        pub unbounded: unbounded::LeafSubCircuit,
        pub circuit: CircuitData<F, C, D>,
    }

    impl DummyLeafCircuit {
        #[must_use]
        pub fn new(circuit_config: &CircuitConfig) -> Self {
            let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());

            let make_tree_inputs = SubCircuitInputs::default(&mut builder);

            let make_tree_targets = make_tree_inputs.build_leaf(&mut builder);
            let (circuit, unbounded) = unbounded::LeafSubCircuit::new(builder);

            let make_tree = make_tree_targets.build(&circuit.prover_only.public_inputs);

            Self {
                make_tree,
                unbounded,
                circuit,
            }
        }

        pub fn prove(
            &self,
            present: bool,
            leaf_value: HashOut<F>,
            branch: &DummyBranchCircuit,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.make_tree.set_witness(&mut inputs, present, leaf_value);
            self.unbounded.set_witness(&mut inputs, &branch.circuit);
            self.circuit.prove(inputs)
        }

        fn prove_unsafe(
            &self,
            hash: HashOut<F>,
            leaf_value: HashOut<F>,
            branch: &DummyBranchCircuit,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.make_tree
                .set_witness_unsafe(&mut inputs, hash, leaf_value);
            self.unbounded.set_witness(&mut inputs, &branch.circuit);
            self.circuit.prove(inputs)
        }
    }

    pub struct DummyBranchCircuit {
        pub make_tree: BranchSubCircuit,
        pub unbounded: unbounded::BranchSubCircuit,
        pub circuit: CircuitData<F, C, D>,
        pub targets: DummyBranchTargets,
    }

    pub struct DummyBranchTargets {
        pub left_proof: ProofWithPublicInputsTarget<D>,
        pub right_proof: ProofWithPublicInputsTarget<D>,
    }

    impl DummyBranchCircuit {
        #[must_use]
        pub fn new(circuit_config: &CircuitConfig, leaf: &DummyLeafCircuit) -> Self {
            let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());
            let common = &leaf.circuit.common;
            let left_proof = builder.add_virtual_proof_with_pis(common);
            let right_proof = builder.add_virtual_proof_with_pis(common);

            let make_tree_inputs = SubCircuitInputs::default(&mut builder);

            let make_tree_targets = make_tree_inputs.build_branch(
                &mut builder,
                &leaf.make_tree,
                &left_proof,
                &right_proof,
            );
            let (circuit, unbounded) = unbounded::BranchSubCircuit::new(
                builder,
                &leaf.circuit,
                make_tree_targets.left_is_leaf,
                make_tree_targets.right_is_leaf,
                &left_proof,
                &right_proof,
            );

            let targets = DummyBranchTargets {
                left_proof,
                right_proof,
            };
            let make_tree =
                make_tree_targets.build(&leaf.make_tree, &circuit.prover_only.public_inputs);

            Self {
                make_tree,
                unbounded,
                circuit,
                targets,
            }
        }

        pub fn prove(
            &self,
            hash: HashOut<F>,
            leaf_value: HashOut<F>,
            left_proof: &ProofWithPublicInputs<F, C, D>,
            right_proof: &ProofWithPublicInputs<F, C, D>,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.make_tree.set_witness(&mut inputs, hash, leaf_value);
            inputs.set_proof_with_pis_target(&self.targets.left_proof, left_proof);
            inputs.set_proof_with_pis_target(&self.targets.right_proof, right_proof);
            self.circuit.prove(inputs)
        }
    }

    #[test]
    fn verify_leaf() -> Result<()> {
        let circuit_config = CircuitConfig::standard_recursion_config();
        let leaf = DummyLeafCircuit::new(&circuit_config);
        let branch = DummyBranchCircuit::new(&circuit_config, &leaf);

        let non_zero_hash = hash_str("Non-Zero Hash");

        let proof = leaf.prove(true, non_zero_hash, &branch)?;
        leaf.circuit.verify(proof)?;

        let proof = leaf.prove(false, non_zero_hash, &branch)?;
        leaf.circuit.verify(proof)?;

        Ok(())
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_all_zero_leaf() {
        let circuit_config = CircuitConfig::standard_recursion_config();
        let leaf = DummyLeafCircuit::new(&circuit_config);
        let branch = DummyBranchCircuit::new(&circuit_config, &leaf);

        let zero_hash = HashOut::from([F::ZERO; NUM_HASH_OUT_ELTS]);

        let proof = leaf.prove(false, zero_hash, &branch).unwrap();
        leaf.circuit.verify(proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_zero_leaf() {
        let circuit_config = CircuitConfig::standard_recursion_config();
        let leaf = DummyLeafCircuit::new(&circuit_config);
        let branch = DummyBranchCircuit::new(&circuit_config, &leaf);

        let zero_hash = HashOut::from([F::ZERO; NUM_HASH_OUT_ELTS]);
        let non_zero_hash = hash_str("Non-Zero Hash");

        let proof = leaf
            .prove_unsafe(non_zero_hash, zero_hash, &branch)
            .unwrap();
        leaf.circuit.verify(proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_leaf_mismatch() {
        let circuit_config = CircuitConfig::standard_recursion_config();
        let leaf = DummyLeafCircuit::new(&circuit_config);
        let branch = DummyBranchCircuit::new(&circuit_config, &leaf);

        let non_zero_hash_1 = hash_str("Non-Zero Hash 1");
        let non_zero_hash_2 = hash_str("Non-Zero Hash 2");

        let proof = leaf
            .prove_unsafe(non_zero_hash_1, non_zero_hash_2, &branch)
            .unwrap();
        leaf.circuit.verify(proof).unwrap();
    }

    #[test]
    fn verify_branch() -> Result<()> {
        let circuit_config = CircuitConfig::standard_recursion_config();
        let leaf = DummyLeafCircuit::new(&circuit_config);
        let branch = DummyBranchCircuit::new(&circuit_config, &leaf);

        let non_zero_hash = hash_str("Non-Zero Hash");
        let branch_hash = hash_branch(&non_zero_hash, &non_zero_hash);
        let branch_hash_1 = hash_branch(&non_zero_hash, &branch_hash);

        let leaf_1_proof = leaf.prove(false, non_zero_hash, &branch)?;
        leaf.circuit.verify(leaf_1_proof.clone())?;

        let leaf_2_proof = leaf.prove(true, non_zero_hash, &branch)?;
        leaf.circuit.verify(leaf_2_proof.clone())?;

        let branch_proof_1 =
            branch.prove(non_zero_hash, non_zero_hash, &leaf_1_proof, &leaf_2_proof)?;
        branch.circuit.verify(branch_proof_1)?;

        let branch_proof_2 =
            branch.prove(branch_hash, non_zero_hash, &leaf_2_proof, &leaf_2_proof)?;
        branch.circuit.verify(branch_proof_2.clone())?;

        let double_branch_proof =
            branch.prove(branch_hash_1, non_zero_hash, &leaf_2_proof, &branch_proof_2)?;
        branch.circuit.verify(double_branch_proof)?;

        Ok(())
    }
}

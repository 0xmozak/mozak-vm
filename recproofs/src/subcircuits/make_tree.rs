//! Subcircuits for recursively proving the construction of a binary merkle tree
//! out of a single value.

use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::{HashOut, HashOutTarget, RichField};
use plonky2::iop::target::{BoolTarget, Target};
use plonky2::iop::witness::{PartialWitness, WitnessWrite};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::proof::ProofWithPublicInputsTarget;

use crate::indices::HashTargetIndex;
use crate::{hash_is_nonzero, hash_is_zero, hash_or_forward, hashes_equal};

/// The indices of the public inputs of this subcircuit in any
/// `ProofWithPublicInputs`
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct PublicIndices {
    /// The indices of each of the elements of the hash
    pub hash: HashTargetIndex,

    /// The indices of each of the elements of the `leaf_value`
    pub leaf_value: HashTargetIndex,
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
        // Find the indices
        let indices = PublicIndices {
            hash: HashTargetIndex::new(public_inputs, self.inputs.hash),
            leaf_value: HashTargetIndex::new(public_inputs, self.inputs.leaf_value),
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
        indices: &PublicIndices,
        left_proof: &ProofWithPublicInputsTarget<D>,
        right_proof: &ProofWithPublicInputsTarget<D>,
    ) -> BranchTargets
    where
        F: RichField + Extendable<D>, {
        let l_hash = indices.hash.get_any(&left_proof.public_inputs);
        let r_hash = indices.hash.get_any(&right_proof.public_inputs);

        // Get presence
        let left_non_zero = hash_is_nonzero(builder, l_hash);
        let right_non_zero = hash_is_nonzero(builder, r_hash);

        // Select the hash based on presence
        let summary_hash = hash_or_forward(builder, left_non_zero, l_hash, right_non_zero, r_hash);
        builder.connect_hashes(self.hash, summary_hash);

        // Make sure the leaf values are the same
        let l_leaf = indices.leaf_value.get(&left_proof.public_inputs);
        let r_leaf = indices.leaf_value.get(&right_proof.public_inputs);
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
    pub fn build(self, child: &PublicIndices, public_inputs: &[Target]) -> BranchSubCircuit {
        // Find the indices
        let indices = PublicIndices {
            hash: HashTargetIndex::new(public_inputs, self.inputs.hash),
            leaf_value: HashTargetIndex::new(public_inputs, self.inputs.leaf_value),
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
        pub make_tree: LeafSubCircuit,
        pub circuit: CircuitData<F, C, D>,
    }

    impl DummyLeafCircuit {
        #[must_use]
        pub fn new(circuit_config: &CircuitConfig) -> Self {
            let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());

            let bounded_inputs = bounded::SubCircuitInputs::default(&mut builder);
            let make_tree_inputs = SubCircuitInputs::default(&mut builder);

            let bounded_targets = bounded_inputs.build_leaf(&mut builder);
            let make_tree_targets = make_tree_inputs.build_leaf(&mut builder);

            let circuit = builder.build();

            let public_inputs = &circuit.prover_only.public_inputs;
            let bounded = bounded_targets.build(public_inputs);
            let make_tree = make_tree_targets.build(public_inputs);

            Self {
                bounded,
                make_tree,
                circuit,
            }
        }

        pub fn prove(
            &self,
            present: bool,
            leaf_value: HashOut<F>,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.bounded.set_witness(&mut inputs);
            self.make_tree.set_witness(&mut inputs, present, leaf_value);
            self.circuit.prove(inputs)
        }

        fn prove_unsafe(
            &self,
            hash: HashOut<F>,
            leaf_value: HashOut<F>,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.bounded.set_witness(&mut inputs);
            self.make_tree
                .set_witness_unsafe(&mut inputs, hash, leaf_value);
            self.circuit.prove(inputs)
        }
    }

    pub struct DummyBranchCircuit {
        pub bounded: bounded::BranchSubCircuit<D>,
        pub make_tree: BranchSubCircuit,
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
            let make_tree_inputs = SubCircuitInputs::default(&mut builder);

            let bounded_targets = bounded_inputs.build_branch(&mut builder, child);
            let make_tree_targets = make_tree_inputs.build_branch(
                &mut builder,
                indices,
                &bounded_targets.left_proof,
                &bounded_targets.right_proof,
            );

            let circuit = builder.build();

            let public_inputs = &circuit.prover_only.public_inputs;
            let bounded = bounded_targets.build(public_inputs);
            let make_tree = make_tree_targets.build(indices, public_inputs);

            Self {
                bounded,
                make_tree,
                circuit,
            }
        }

        #[must_use]
        pub fn from_leaf(circuit_config: &CircuitConfig, leaf: &DummyLeafCircuit) -> Self {
            Self::new(circuit_config, &leaf.make_tree.indices, &leaf.circuit)
        }

        #[must_use]
        pub fn from_branch(circuit_config: &CircuitConfig, branch: &Self) -> Self {
            Self::new(circuit_config, &branch.make_tree.indices, &branch.circuit)
        }

        pub fn prove(
            &self,
            hash: HashOut<F>,
            leaf_value: HashOut<F>,
            left_proof: &ProofWithPublicInputs<F, C, D>,
            right_proof: &ProofWithPublicInputs<F, C, D>,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.bounded
                .set_witness(&mut inputs, left_proof, right_proof);
            self.make_tree.set_witness(&mut inputs, hash, leaf_value);
            self.circuit.prove(inputs)
        }
    }

    lazy_static! {
        static ref LEAF: DummyLeafCircuit = DummyLeafCircuit::new(&CONFIG);
        static ref BRANCH_1: DummyBranchCircuit = DummyBranchCircuit::from_leaf(&CONFIG, &LEAF);
        static ref BRANCH_2: DummyBranchCircuit =
            DummyBranchCircuit::from_branch(&CONFIG, &BRANCH_1);
    }

    #[test]
    fn verify_leaf() -> Result<()> {
        let non_zero_hash = hash_str("Non-Zero Hash");

        let proof = LEAF.prove(true, non_zero_hash)?;
        LEAF.circuit.verify(proof)?;

        let proof = LEAF.prove(false, non_zero_hash)?;
        LEAF.circuit.verify(proof)?;

        Ok(())
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_all_zero_leaf() {
        let zero_hash = HashOut::from([F::ZERO; NUM_HASH_OUT_ELTS]);

        let proof = LEAF.prove(false, zero_hash).unwrap();
        LEAF.circuit.verify(proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_zero_leaf() {
        let zero_hash = HashOut::from([F::ZERO; NUM_HASH_OUT_ELTS]);
        let non_zero_hash = hash_str("Non-Zero Hash");

        let proof = LEAF.prove_unsafe(non_zero_hash, zero_hash).unwrap();
        LEAF.circuit.verify(proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_leaf_mismatch() {
        let non_zero_hash_1 = hash_str("Non-Zero Hash 1");
        let non_zero_hash_2 = hash_str("Non-Zero Hash 2");

        let proof = LEAF.prove_unsafe(non_zero_hash_1, non_zero_hash_2).unwrap();
        LEAF.circuit.verify(proof).unwrap();
    }

    #[test]
    fn verify_branch() -> Result<()> {
        let non_zero_hash = hash_str("Non-Zero Hash");
        let branch_hash = hash_branch(&non_zero_hash, &non_zero_hash);
        let branch_hash_1 = hash_branch(&non_zero_hash, &branch_hash);

        let leaf_1_proof = LEAF.prove(false, non_zero_hash)?;
        LEAF.circuit.verify(leaf_1_proof.clone())?;

        let leaf_2_proof = LEAF.prove(true, non_zero_hash)?;
        LEAF.circuit.verify(leaf_2_proof.clone())?;

        let branch_proof_1 =
            BRANCH_1.prove(non_zero_hash, non_zero_hash, &leaf_1_proof, &leaf_2_proof)?;
        BRANCH_1.circuit.verify(branch_proof_1.clone())?;

        let branch_proof_2 =
            BRANCH_1.prove(branch_hash, non_zero_hash, &leaf_2_proof, &leaf_2_proof)?;
        BRANCH_1.circuit.verify(branch_proof_2.clone())?;

        let double_branch_proof = BRANCH_2.prove(
            branch_hash_1,
            non_zero_hash,
            &branch_proof_1,
            &branch_proof_2,
        )?;
        BRANCH_2.circuit.verify(double_branch_proof)?;

        Ok(())
    }
}

//! Subcircuits for recursively proving the construction of a binary merkle tree
//! out of a single value.
//!
//! These subcircuits are recursive, building on top of each other to
//! create the next level up of the merkle tree.
use iter_fixed::IntoIteratorFixed;
use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::{HashOut, HashOutTarget, RichField, NUM_HASH_OUT_ELTS};
use plonky2::hash::poseidon2::Poseidon2Hash;
use plonky2::iop::target::BoolTarget;
use plonky2::iop::witness::{PartialWitness, WitnessWrite};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::CircuitData;
use plonky2::plonk::config::GenericConfig;
use plonky2::plonk::proof::ProofWithPublicInputsTarget;

use super::select_hash;

/// Computes `h0 == h1`.
fn and_helper<F, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    bools: [BoolTarget; NUM_HASH_OUT_ELTS],
) -> BoolTarget
where
    F: RichField + Extendable<D>, {
    let bools = [
        builder.and(bools[0], bools[1]),
        builder.and(bools[2], bools[3]),
    ];
    builder.and(bools[0], bools[1])
}

/// Computes `h0 == h1`.
fn hashes_equal<F, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    h0: HashOutTarget,
    h1: HashOutTarget,
) -> BoolTarget
where
    F: RichField + Extendable<D>, {
    let eq = h0
        .elements
        .into_iter_fixed()
        .zip(h1.elements)
        .map(|(h0, h1)| builder.is_equal(h0, h1))
        .collect();
    and_helper(builder, eq)
}

/// Computes `h0 == ZERO`.
fn hash_is_zero<F, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    h0: HashOutTarget,
) -> BoolTarget
where
    F: RichField + Extendable<D>, {
    let zero = h0
        .elements
        .into_iter_fixed()
        .map(|h0| {
            let non_zero = builder.is_nonzero(h0);
            builder.not(non_zero)
        })
        .collect();
    and_helper(builder, zero)
}

/// The indices of the public inputs of this subcircuit in any
/// `ProofWithPublicInputs`
#[derive(Copy, Clone)]
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

pub struct LeafTargets {
    /// The leaf value or ZERO if absent
    pub hash: HashOutTarget,

    /// The value to be progated throughout the produced tree
    pub leaf_value: HashOutTarget,
}

/// The leaf subcircuit metadata. This subcircuit does basically nothing, simply
/// expressing that a hash exists
pub struct LeafSubCircuit {
    pub targets: LeafTargets,
    pub indices: PublicIndices,
}

impl LeafSubCircuit {
    #[must_use]
    pub fn new<F, C, const D: usize, B, R>(
        mut builder: CircuitBuilder<F, D>,
        build: B,
    ) -> (CircuitData<F, C, D>, (Self, R))
    where
        B: FnOnce(&LeafTargets, CircuitBuilder<F, D>) -> (CircuitData<F, C, D>, R),
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F>, {
        let one = builder.one();
        let hash = builder.add_virtual_hash();
        let leaf_value = builder.add_virtual_hash();
        builder.register_public_inputs(&hash.elements);
        builder.register_public_inputs(&leaf_value.elements);

        let eq = hashes_equal(&mut builder, hash, leaf_value);
        let zero = hash_is_zero(&mut builder, hash);

        let xor = builder.add(eq.target, zero.target);
        builder.connect(xor, one);

        let targets = LeafTargets { hash, leaf_value };

        // Build the circuit
        let (circuit, r) = build(&targets, builder);

        // Find the indicies
        let indices = PublicIndices {
            hash: targets.hash.elements.map(|target| {
                circuit
                    .prover_only
                    .public_inputs
                    .iter()
                    .position(|&pi| pi == target)
                    .expect("target not found")
            }),
            leaf_value: targets.leaf_value.elements.map(|target| {
                circuit
                    .prover_only
                    .public_inputs
                    .iter()
                    .position(|&pi| pi == target)
                    .expect("target not found")
            }),
        };
        let v = Self { targets, indices };

        (circuit, (v, r))
    }

    /// Get ready to generate a proof
    pub fn set_inputs<F: RichField>(
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
        self.set_inputs_unsafe(inputs, hash, leaf_value);
    }

    pub fn set_inputs_unsafe<F: RichField>(
        &self,
        inputs: &mut PartialWitness<F>,
        hash: HashOut<F>,
        leaf_value: HashOut<F>,
    ) {
        inputs.set_hash_target(self.targets.hash, hash);
        inputs.set_hash_target(self.targets.leaf_value, leaf_value);
    }
}

pub struct BranchSubCircuit {
    pub targets: BranchTargets,
    pub indices: PublicIndices,
}

pub struct BranchTargets {
    /// The leaf value or ZERO if absent
    pub hash: HashOutTarget,

    /// The value to be progated throughout the produced tree
    pub leaf_value: HashOutTarget,

    /// Indicates if the left branch is a leaf or not
    pub left_is_leaf: BoolTarget,

    /// Indicates if the right branch is a leaf or not
    pub right_is_leaf: BoolTarget,
}

impl BranchSubCircuit {
    #[must_use]
    pub fn new<F, C, const D: usize, B, R>(
        mut builder: CircuitBuilder<F, D>,
        leaf: &LeafSubCircuit,
        left_proof: &ProofWithPublicInputsTarget<D>,
        right_proof: &ProofWithPublicInputsTarget<D>,
        build: B,
    ) -> (CircuitData<F, C, D>, (Self, R))
    where
        B: FnOnce(&BranchTargets, CircuitBuilder<F, D>) -> (CircuitData<F, C, D>, R),
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F>, {
        let hash = builder.add_virtual_hash();
        let leaf_value = builder.add_virtual_hash();
        builder.register_public_inputs(&hash.elements);
        builder.register_public_inputs(&leaf_value.elements);

        let l_hash = leaf.indices.get_hash(&left_proof.public_inputs);
        let r_hash = leaf.indices.get_hash(&right_proof.public_inputs);

        let left_non_zero: [_; NUM_HASH_OUT_ELTS] = l_hash
            .into_iter_fixed()
            .map(|l_hash| builder.is_nonzero(l_hash))
            .collect();
        let right_non_zero: [_; NUM_HASH_OUT_ELTS] = r_hash
            .into_iter_fixed()
            .map(|r_hash| builder.is_nonzero(r_hash))
            .collect();

        let left_non_zero = [
            builder.or(left_non_zero[0], left_non_zero[1]),
            builder.or(left_non_zero[2], left_non_zero[3]),
        ];
        let left_non_zero = builder.or(left_non_zero[0], left_non_zero[1]);

        let right_non_zero = [
            builder.or(right_non_zero[0], right_non_zero[1]),
            builder.or(right_non_zero[2], right_non_zero[3]),
        ];
        let right_non_zero = builder.or(right_non_zero[0], right_non_zero[1]);

        let both_present = builder.and(left_non_zero, right_non_zero);

        // Construct the hash of [left, right]
        let hash_both = builder
            .hash_n_to_hash_no_pad::<Poseidon2Hash>(l_hash.into_iter().chain(r_hash).collect());

        // Construct the forwarding "hash".
        let hash_absent = l_hash
            .into_iter_fixed()
            .zip(r_hash)
            // Since absent sides will be zero, we can just sum.
            .map(|(l, r)| builder.add(l, r))
            .collect();
        let hash_absent = HashOutTarget {
            elements: hash_absent,
        };

        // Select the hash based on presence
        let summary_hash = select_hash(&mut builder, both_present, hash_both, hash_absent);
        builder.connect_hashes(hash, summary_hash);

        // Make sure the leaf values are the same
        let l_leaf = leaf.indices.get_leaf_value(&left_proof.public_inputs);
        let r_leaf = leaf.indices.get_leaf_value(&right_proof.public_inputs);
        let l_leaf = HashOutTarget::from(l_leaf);
        let r_leaf = HashOutTarget::from(r_leaf);
        builder.connect_hashes(leaf_value, l_leaf);
        builder.connect_hashes(leaf_value, r_leaf);

        // Determine the type of the proof by looking at its public input
        // This works because only the root is allowed to be a incomplete (one-sided)
        // branch
        let l_hash = HashOutTarget::from(l_hash);
        let left_eq = hashes_equal(&mut builder, l_hash, leaf_value);
        let left_zero = hash_is_zero(&mut builder, l_hash);
        let left_is_leaf = builder.add(left_eq.target, left_zero.target);
        let left_is_leaf = BoolTarget::new_unsafe(left_is_leaf);
        builder.assert_bool(left_is_leaf);

        let r_hash = HashOutTarget::from(r_hash);
        let right_eq = hashes_equal(&mut builder, r_hash, leaf_value);
        let right_zero = hash_is_zero(&mut builder, r_hash);
        let right_is_leaf = builder.add(right_eq.target, right_zero.target);
        let right_is_leaf = BoolTarget::new_unsafe(right_is_leaf);
        builder.assert_bool(right_is_leaf);

        let targets = BranchTargets {
            hash,
            leaf_value,
            left_is_leaf,
            right_is_leaf,
        };
        let (circuit, r) = build(&targets, builder);
        let public_inputs = &circuit.prover_only.public_inputs;

        let indices = PublicIndices {
            hash: targets.hash.elements.map(|target| {
                public_inputs
                    .iter()
                    .position(|&pi| pi == target)
                    .expect("target not found")
            }),
            leaf_value: targets.leaf_value.elements.map(|target| {
                public_inputs
                    .iter()
                    .position(|&pi| pi == target)
                    .expect("target not found")
            }),
        };
        let v = Self { targets, indices };

        (circuit, (v, r))
    }

    pub fn set_inputs<F: RichField>(
        &self,
        inputs: &mut PartialWitness<F>,
        hash: HashOut<F>,
        leaf_value: HashOut<F>,
    ) {
        inputs.set_hash_target(self.targets.hash, hash);
        inputs.set_hash_target(self.targets.leaf_value, leaf_value);
    }
}
#[cfg(test)]
mod test {
    use anyhow::Result;
    use plonky2::field::types::Field;
    use plonky2::hash::hash_types::{HashOut, NUM_HASH_OUT_ELTS};
    use plonky2::plonk::circuit_data::CircuitConfig;
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
            let builder = CircuitBuilder::<F, D>::new(circuit_config.clone());
            let (circuit, (make_tree, (unbounded, ()))) =
                LeafSubCircuit::new(builder, |_targets, builder| {
                    unbounded::LeafSubCircuit::new(builder)
                });

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
            self.make_tree.set_inputs(&mut inputs, present, leaf_value);
            self.unbounded.set_inputs(&mut inputs, &branch.circuit);
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
                .set_inputs_unsafe(&mut inputs, hash, leaf_value);
            self.unbounded.set_inputs(&mut inputs, &branch.circuit);
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
            let (circuit, (make_tree, (unbounded, ()))) = BranchSubCircuit::new(
                builder,
                &leaf.make_tree,
                &left_proof,
                &right_proof,
                |targets, builder| {
                    unbounded::BranchSubCircuit::new(
                        builder,
                        &leaf.circuit,
                        targets.left_is_leaf,
                        targets.right_is_leaf,
                        &left_proof,
                        &right_proof,
                    )
                },
            );

            let targets = DummyBranchTargets {
                left_proof,
                right_proof,
            };
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
            self.make_tree.set_inputs(&mut inputs, hash, leaf_value);
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

//! Subcircuits for recursively proving the merge of two binary merkle trees
//!
//! These subcircuits are recursive, building on top of each other to
//! create the next level up of the merged merkle tree.

use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::{HashOut, HashOutTarget, RichField, NUM_HASH_OUT_ELTS};
use plonky2::iop::target::BoolTarget;
use plonky2::iop::witness::{PartialWitness, WitnessWrite};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::CircuitData;
use plonky2::plonk::config::GenericConfig;
use plonky2::plonk::proof::ProofWithPublicInputsTarget;

use super::{hash_or_forward_zero, hashes_equal};

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

pub struct LeafTargets {
    /// The a hash
    pub a_hash: HashOutTarget,

    /// The b hash
    pub b_hash: HashOutTarget,

    /// The merged hash
    pub merged_hash: HashOutTarget,
}

/// The leaf subcircuit metadata. This subcircuit merges up to two leaf hashes
/// creating a parent node if two leafs are present, otherwise just forwarding
/// any existing hash
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
        let a_hash = builder.add_virtual_hash();
        let b_hash = builder.add_virtual_hash();

        let a_elem = a_hash.elements;
        let b_elem = b_hash.elements;
        let merged_hash = hash_or_forward_zero(&mut builder, a_elem, b_elem);

        // Register public inputs
        builder.register_public_inputs(&a_elem);
        builder.register_public_inputs(&b_elem);
        builder.register_public_inputs(&merged_hash.elements);

        let targets = LeafTargets {
            a_hash,
            b_hash,
            merged_hash,
        };
        let (circuit, r) = build(&targets, builder);
        let public_inputs = &circuit.prover_only.public_inputs;

        let indices = PublicIndices {
            a_hash: targets.a_hash.elements.map(|target| {
                public_inputs
                    .iter()
                    .position(|&pi| pi == target)
                    .expect("target not found")
            }),
            b_hash: targets.b_hash.elements.map(|target| {
                public_inputs
                    .iter()
                    .position(|&pi| pi == target)
                    .expect("target not found")
            }),
            merged_hash: targets.merged_hash.elements.map(|target| {
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
        a_hash: HashOut<F>,
        b_hash: HashOut<F>,
        merged_hash: Option<HashOut<F>>,
    ) {
        inputs.set_hash_target(self.targets.a_hash, a_hash);
        inputs.set_hash_target(self.targets.b_hash, b_hash);
        if let Some(merged_hash) = merged_hash {
            inputs.set_hash_target(self.targets.merged_hash, merged_hash);
        }
    }
}

pub struct BranchTargets {
    /// The a hash
    pub a_hash: HashOutTarget,

    /// The b hash
    pub b_hash: HashOutTarget,

    /// The merged hash
    pub merged_hash: HashOutTarget,

    /// Indicates if the left branch is a leaf or not
    pub left_is_leaf: BoolTarget,

    /// Indicates if the right branch is a leaf or not
    pub right_is_leaf: BoolTarget,
}

/// The branch subcircuit metadata. This subcircuit merges up to two leaf hashes
/// creating a parent node if two leafs are present, otherwise just forwarding
/// any existing hash
pub struct BranchSubCircuit {
    pub targets: BranchTargets,
    pub indices: PublicIndices,
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
        let left_a = leaf.indices.get_a_hash(&left_proof.public_inputs);
        let left_b = leaf.indices.get_b_hash(&left_proof.public_inputs);
        let left_merged = leaf.indices.get_merged_hash(&left_proof.public_inputs);
        let left_is_leaf = {
            let left_ab_hash = hash_or_forward_zero(&mut builder, left_a, left_b);
            hashes_equal(&mut builder, left_merged.into(), left_ab_hash)
        };

        let right_a = leaf.indices.get_a_hash(&right_proof.public_inputs);
        let right_b = leaf.indices.get_b_hash(&right_proof.public_inputs);
        let right_merged = leaf.indices.get_merged_hash(&right_proof.public_inputs);
        let right_is_leaf = {
            let right_ab_hash = hash_or_forward_zero(&mut builder, right_a, right_b);
            hashes_equal(&mut builder, right_merged.into(), right_ab_hash)
        };

        let a_hash = hash_or_forward_zero(&mut builder, left_a, right_a);
        let b_hash = hash_or_forward_zero(&mut builder, left_b, right_b);
        let merged_hash = hash_or_forward_zero(&mut builder, left_merged, right_merged);

        // Register public inputs
        builder.register_public_inputs(&a_hash.elements);
        builder.register_public_inputs(&b_hash.elements);
        builder.register_public_inputs(&merged_hash.elements);

        let targets = BranchTargets {
            a_hash,
            b_hash,
            merged_hash,
            left_is_leaf,
            right_is_leaf,
        };

        let (circuit, r) = build(&targets, builder);
        let public_inputs = &circuit.prover_only.public_inputs;

        let indices = PublicIndices {
            a_hash: targets.a_hash.elements.map(|target| {
                public_inputs
                    .iter()
                    .position(|&pi| pi == target)
                    .expect("target not found")
            }),
            b_hash: targets.b_hash.elements.map(|target| {
                public_inputs
                    .iter()
                    .position(|&pi| pi == target)
                    .expect("target not found")
            }),
            merged_hash: targets.merged_hash.elements.map(|target| {
                public_inputs
                    .iter()
                    .position(|&pi| pi == target)
                    .expect("target not found")
            }),
        };
        assert_eq!(indices, leaf.indices);
        let v = Self { targets, indices };

        (circuit, (v, r))
    }

    pub fn set_inputs<F: RichField>(
        &self,
        inputs: &mut PartialWitness<F>,
        a_hash: HashOut<F>,
        b_hash: HashOut<F>,
        merged_hash: Option<HashOut<F>>,
    ) {
        inputs.set_hash_target(self.targets.a_hash, a_hash);
        inputs.set_hash_target(self.targets.b_hash, b_hash);
        if let Some(merged_hash) = merged_hash {
            inputs.set_hash_target(self.targets.merged_hash, merged_hash);
        }
    }
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use plonky2::field::types::Field;
    use plonky2::hash::hash_types::{HashOut, NUM_HASH_OUT_ELTS};
    use plonky2::plonk::circuit_data::CircuitConfig;
    use plonky2::plonk::proof::{ProofWithPublicInputs, ProofWithPublicInputsTarget};

    use super::*;
    use crate::recproof::unbounded;
    use crate::test_utils::{hash_branch, hash_str, C, D, F};

    pub struct DummyLeafCircuit {
        pub merge: LeafSubCircuit,
        pub unbounded: unbounded::LeafSubCircuit,
        pub circuit: CircuitData<F, C, D>,
    }

    impl DummyLeafCircuit {
        #[must_use]
        pub fn new(circuit_config: &CircuitConfig) -> Self {
            let builder = CircuitBuilder::<F, D>::new(circuit_config.clone());
            let (circuit, (merge, (unbounded, ()))) =
                LeafSubCircuit::new(builder, |_targets, builder| {
                    unbounded::LeafSubCircuit::new(builder)
                });

            Self {
                merge,
                unbounded,
                circuit,
            }
        }

        pub fn prove(
            &self,
            a_tree: HashOut<F>,
            b_tree: HashOut<F>,
            merged_hash: Option<HashOut<F>>,
            branch: &DummyBranchCircuit,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.merge
                .set_inputs(&mut inputs, a_tree, b_tree, merged_hash);
            self.unbounded.set_inputs(&mut inputs, &branch.circuit);
            self.circuit.prove(inputs)
        }
    }

    pub struct DummyBranchCircuit {
        pub merge: BranchSubCircuit,
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
            let (circuit, (merge, (unbounded, ()))) = BranchSubCircuit::new(
                builder,
                &leaf.merge,
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
                merge,
                unbounded,
                circuit,
                targets,
            }
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
            self.merge
                .set_inputs(&mut inputs, a_tree, b_tree, merged_hash);
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

        let zero_hash = HashOut::from([F::ZERO; NUM_HASH_OUT_ELTS]);
        let a_val = hash_str("Value Alpha");
        let b_val = hash_str("Value Beta");
        let ab_hash = hash_branch(&a_val, &b_val);

        let proof = leaf.prove(zero_hash, zero_hash, Some(zero_hash), &branch)?;
        leaf.circuit.verify(proof)?;

        let proof = leaf.prove(a_val, zero_hash, Some(a_val), &branch)?;
        leaf.circuit.verify(proof)?;

        let proof = leaf.prove(zero_hash, b_val, Some(b_val), &branch)?;
        leaf.circuit.verify(proof)?;

        let proof = leaf.prove(a_val, b_val, Some(ab_hash), &branch)?;
        leaf.circuit.verify(proof)?;

        Ok(())
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
            .prove(zero_hash, zero_hash, Some(non_zero_hash), &branch)
            .unwrap();
        leaf.circuit.verify(proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_leaf_mismatch() {
        let circuit_config = CircuitConfig::standard_recursion_config();
        let leaf = DummyLeafCircuit::new(&circuit_config);
        let branch = DummyBranchCircuit::new(&circuit_config, &leaf);

        let zero_hash = HashOut::from([F::ZERO; NUM_HASH_OUT_ELTS]);
        let non_zero_hash_1 = hash_str("Non-Zero Hash 1");
        let non_zero_hash_2 = hash_str("Non-Zero Hash 2");

        let proof = leaf
            .prove(non_zero_hash_1, zero_hash, Some(non_zero_hash_2), &branch)
            .unwrap();
        leaf.circuit.verify(proof).unwrap();
    }

    #[test]
    #[allow(clippy::similar_names)]
    fn verify_branch() -> Result<()> {
        let circuit_config = CircuitConfig::standard_recursion_config();
        let leaf = DummyLeafCircuit::new(&circuit_config);
        let branch = DummyBranchCircuit::new(&circuit_config, &leaf);

        let zero_hash = HashOut::from([F::ZERO; NUM_HASH_OUT_ELTS]);
        let a_val = hash_str("Value Alpha");
        let b_val = hash_str("Value Beta");
        let ab_hash = hash_branch(&a_val, &b_val);

        let a_proof = leaf.prove(a_val, zero_hash, Some(a_val), &branch)?;
        leaf.circuit.verify(a_proof.clone())?;

        let b_proof = leaf.prove(zero_hash, b_val, Some(b_val), &branch)?;
        leaf.circuit.verify(b_proof.clone())?;

        // In practice, you should never merge two single value leafs because doing so
        // results in a terminal proof which can't be recursed
        let proof = branch.prove(a_val, b_val, Some(ab_hash), &a_proof, &b_proof)?;
        branch.circuit.verify(proof.clone())?;

        // Test that multi-value leafs work
        let empty_proof = leaf.prove(zero_hash, zero_hash, Some(zero_hash), &branch)?;
        leaf.circuit.verify(empty_proof.clone())?;

        let ab_proof = leaf.prove(a_val, b_val, Some(ab_hash), &branch)?;
        leaf.circuit.verify(ab_proof.clone())?;

        let proof = branch.prove(a_val, b_val, Some(ab_hash), &ab_proof, &empty_proof)?;
        branch.circuit.verify(proof)?;

        Ok(())
    }

    #[test]
    #[allow(clippy::similar_names)]
    fn verify_branch2() -> Result<()> {
        let circuit_config = CircuitConfig::standard_recursion_config();
        let leaf = DummyLeafCircuit::new(&circuit_config);
        let branch = DummyBranchCircuit::new(&circuit_config, &leaf);

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

        let empty_proof = leaf.prove(zero_hash, zero_hash, Some(zero_hash), &branch)?;
        leaf.circuit.verify(empty_proof.clone())?;

        let ab_proof = leaf.prove(a_val, b_val, Some(ab_hash), &branch)?;
        leaf.circuit.verify(ab_proof.clone())?;

        let cd_proof = leaf.prove(c_val, d_val, Some(cd_hash), &branch)?;
        leaf.circuit.verify(cd_proof.clone())?;

        let mut abcd_proof =
            branch.prove(ac_hash, bd_hash, Some(abcd_hash), &ab_proof, &cd_proof)?;
        branch.circuit.verify(abcd_proof.clone())?;

        // Test that empty leafs have no effect
        for _ in 0..4 {
            abcd_proof =
                branch.prove(ac_hash, bd_hash, Some(abcd_hash), &abcd_proof, &empty_proof)?;
            branch.circuit.verify(abcd_proof.clone())?;
        }

        Ok(())
    }
}

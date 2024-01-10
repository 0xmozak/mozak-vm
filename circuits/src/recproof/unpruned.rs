//! Subcircuits for recursively proving the entire contents of a merkle tree
//!
//! These subcircuits are pseudo-recursive, building on top of each other to
//! create the next level up of the merkle tree. "Pseudo-" here means the height
//! must be fixed ahead of time and not depend on the content.
//!
//! These subcircuits are useful because with just a pair of them, say a old and
//! new, you can prove a transition from the current merkle root (proved by old)
//! to a new merkle root (proved by new).
use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::{HashOut, HashOutTarget, RichField, NUM_HASH_OUT_ELTS};
use plonky2::hash::poseidon2::Poseidon2Hash;
use plonky2::iop::witness::{PartialWitness, WitnessWrite};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::CircuitData;
use plonky2::plonk::config::GenericConfig;
use plonky2::plonk::proof::ProofWithPublicInputsTarget;

/// The indices of the public inputs of this subcircuit in any
/// `ProofWithPublicInputs`
#[derive(Copy, Clone)]
pub struct PublicIndices {
    /// The indices of each of the elements of the unpruned hash
    pub unpruned_hash: [usize; NUM_HASH_OUT_ELTS],
}

impl PublicIndices {
    /// Extract unpruned hash from an array of public inputs.
    pub fn get_unpruned_hash<T: Copy>(&self, public_inputs: &[T]) -> [T; NUM_HASH_OUT_ELTS] {
        self.unpruned_hash.map(|i| public_inputs[i])
    }

    /// Insert unpruned hash into an array of public inputs.
    pub fn set_unpruned_hash<T>(&self, public_inputs: &mut [T], v: [T; NUM_HASH_OUT_ELTS]) {
        for (i, v) in v.into_iter().enumerate() {
            public_inputs[self.unpruned_hash[i]] = v;
        }
    }
}

/// The leaf subcircuit metadata. This subcircuit does basically nothing, simply
/// expressing that a hash exists
pub struct LeafSubCircuit {
    pub targets: LeafTargets,
    pub indices: PublicIndices,
}

pub struct LeafTargets {
    /// The hash of the unpruned state or ZERO if absent
    pub unpruned_hash: HashOutTarget,
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
        let unpruned_hash = builder.add_virtual_hash();
        builder.register_public_inputs(&unpruned_hash.elements);

        let targets = LeafTargets { unpruned_hash };

        // Build the circuit
        let (circuit, r) = build(&targets, builder);

        // Find the indicies
        let indices = PublicIndices {
            unpruned_hash: targets.unpruned_hash.elements.map(|target| {
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
        unpruned_hash: HashOut<F>,
    ) {
        inputs.set_hash_target(self.targets.unpruned_hash, unpruned_hash);
    }
}

/// The branch subcircuit metadata. This subcircuit proves knowledge of two
/// private subcircuit proofs, and that the public `unpruned_hash` values of
/// those circuits hash together to the public `unpruned_hash` value of this
/// circuit.
pub struct BranchSubCircuit {
    pub targets: BranchTargets,
    pub indices: PublicIndices,
    /// The distance from the leaves (`0`` being the lowest branch)
    /// Used for debugging
    pub height: usize,
}

pub struct BranchTargets {
    /// The left dir
    pub left_dir: BranchDirTargets,

    /// The right dir
    pub right_dir: BranchDirTargets,

    /// The hash of `[left.unpruned_hash, right.unpruned_hash]`
    pub unpruned_hash: HashOutTarget,
}

pub struct BranchDirTargets {
    /// The hash of this dir proved by `proof`
    pub unpruned_hash: HashOutTarget,
}

impl BranchSubCircuit {
    fn from_dirs<F, C, const D: usize, B, R>(
        mut builder: CircuitBuilder<F, D>,
        left_dir: BranchDirTargets,
        right_dir: BranchDirTargets,
        height: usize,
        build: B,
    ) -> (CircuitData<F, C, D>, (Self, R))
    where
        B: FnOnce(&BranchTargets, CircuitBuilder<F, D>) -> (CircuitData<F, C, D>, R),
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F>, {
        // Hash the left and right together
        let unpruned_hash = builder.hash_n_to_hash_no_pad::<Poseidon2Hash>(
            left_dir
                .unpruned_hash
                .elements
                .into_iter()
                .chain(right_dir.unpruned_hash.elements)
                .collect(),
        );

        // Register the "output"
        builder.register_public_inputs(&unpruned_hash.elements);

        let targets = BranchTargets {
            left_dir,
            right_dir,
            unpruned_hash,
        };

        // Build the circuit
        let (circuit, r) = build(&targets, builder);

        // Find the indicies
        let indices = PublicIndices {
            unpruned_hash: targets.unpruned_hash.elements.map(|target| {
                circuit
                    .prover_only
                    .public_inputs
                    .iter()
                    .position(|&pi| pi == target)
                    .expect("target not found")
            }),
        };
        let v = Self {
            targets,
            indices,
            height,
        };

        (circuit, (v, r))
    }

    fn dir_from_node<const D: usize>(
        proof: &ProofWithPublicInputsTarget<D>,
        indices: &PublicIndices,
    ) -> BranchDirTargets {
        let unpruned_hash = HashOutTarget::from(indices.get_unpruned_hash(&proof.public_inputs));

        BranchDirTargets { unpruned_hash }
    }

    pub fn from_leaf<F, C, const D: usize, B, R>(
        builder: CircuitBuilder<F, D>,
        leaf: &LeafSubCircuit,
        left_proof: &ProofWithPublicInputsTarget<D>,
        right_proof: &ProofWithPublicInputsTarget<D>,
        build: B,
    ) -> (CircuitData<F, C, D>, (Self, R))
    where
        B: FnOnce(&BranchTargets, CircuitBuilder<F, D>) -> (CircuitData<F, C, D>, R),
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F>, {
        let left_dir = Self::dir_from_node(left_proof, &leaf.indices);
        let right_dir = Self::dir_from_node(right_proof, &leaf.indices);
        let height = 0;
        Self::from_dirs(builder, left_dir, right_dir, height, build)
    }

    pub fn from_branch<F, C, const D: usize, B, R>(
        builder: CircuitBuilder<F, D>,
        branch: &BranchSubCircuit,
        left_proof: &ProofWithPublicInputsTarget<D>,
        right_proof: &ProofWithPublicInputsTarget<D>,
        build: B,
    ) -> (CircuitData<F, C, D>, (Self, R))
    where
        B: FnOnce(&BranchTargets, CircuitBuilder<F, D>) -> (CircuitData<F, C, D>, R),
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F>, {
        let left_dir = Self::dir_from_node(left_proof, &branch.indices);
        let right_dir = Self::dir_from_node(right_proof, &branch.indices);
        let height = branch.height + 1;
        Self::from_dirs(builder, left_dir, right_dir, height, build)
    }

    /// Get ready to generate a proof
    pub fn set_inputs<F: RichField>(
        &self,
        inputs: &mut PartialWitness<F>,
        unpruned_hash: HashOut<F>,
    ) {
        inputs.set_hash_target(self.targets.unpruned_hash, unpruned_hash);
    }
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use plonky2::field::types::Field;
    use plonky2::plonk::circuit_data::CircuitConfig;
    use plonky2::plonk::proof::ProofWithPublicInputs;

    use super::*;
    use crate::test_utils::{hash_branch, hash_str, C, D, F};

    pub struct DummyLeafCircuit {
        pub unpruned: LeafSubCircuit,
        pub circuit: CircuitData<F, C, D>,
    }

    impl DummyLeafCircuit {
        #[must_use]
        pub fn new(circuit_config: &CircuitConfig) -> Self {
            let builder = CircuitBuilder::<F, D>::new(circuit_config.clone());
            let (circuit, (unpruned, ())) =
                LeafSubCircuit::new(builder, |_targets, builder| (builder.build(), ()));

            Self { unpruned, circuit }
        }

        pub fn prove(&self, unpruned_hash: HashOut<F>) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.unpruned.set_inputs(&mut inputs, unpruned_hash);
            self.circuit.prove(inputs)
        }
    }

    pub struct DummyBranchCircuit {
        pub summarized: BranchSubCircuit,
        pub circuit: CircuitData<F, C, D>,
        pub targets: DummyBranchTargets,
    }

    pub struct DummyBranchTargets {
        pub left_proof: ProofWithPublicInputsTarget<D>,
        pub right_proof: ProofWithPublicInputsTarget<D>,
    }

    impl DummyBranchCircuit {
        #[must_use]
        pub fn from_leaf(circuit_config: &CircuitConfig, leaf: &DummyLeafCircuit) -> Self {
            let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());

            let circuit_data = &leaf.circuit;
            let common = &circuit_data.common;
            let verifier = builder.constant_verifier_data(&circuit_data.verifier_only);
            let left_proof = builder.add_virtual_proof_with_pis(common);
            let right_proof = builder.add_virtual_proof_with_pis(common);
            builder.verify_proof::<C>(&left_proof, &verifier, common);
            builder.verify_proof::<C>(&right_proof, &verifier, common);

            let (circuit, (summarized, ())) = BranchSubCircuit::from_leaf(
                builder,
                &leaf.unpruned,
                &left_proof,
                &right_proof,
                |_targets, builder| (builder.build(), ()),
            );

            let targets = DummyBranchTargets {
                left_proof,
                right_proof,
            };

            Self {
                summarized,
                circuit,
                targets,
            }
        }

        pub fn from_branch(circuit_config: &CircuitConfig, branch: &DummyBranchCircuit) -> Self {
            let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());

            let circuit_data = &branch.circuit;
            let common = &circuit_data.common;
            let verifier = builder.constant_verifier_data(&circuit_data.verifier_only);
            let left_proof = builder.add_virtual_proof_with_pis(common);
            let right_proof = builder.add_virtual_proof_with_pis(common);
            builder.verify_proof::<C>(&left_proof, &verifier, common);
            builder.verify_proof::<C>(&right_proof, &verifier, common);

            let (circuit, (summarized, ())) = BranchSubCircuit::from_branch(
                builder,
                &branch.summarized,
                &left_proof,
                &right_proof,
                |_targets, builder| (builder.build(), ()),
            );

            let targets = DummyBranchTargets {
                left_proof,
                right_proof,
            };

            Self {
                summarized,
                circuit,
                targets,
            }
        }

        pub fn prove(
            &self,
            left_proof: &ProofWithPublicInputs<F, C, D>,
            right_proof: &ProofWithPublicInputs<F, C, D>,
            unpruned_hash: HashOut<F>,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            inputs.set_proof_with_pis_target(&self.targets.left_proof, left_proof);
            inputs.set_proof_with_pis_target(&self.targets.right_proof, right_proof);
            self.summarized.set_inputs(&mut inputs, unpruned_hash);
            self.circuit.prove(inputs)
        }
    }

    #[test]
    fn verify_leaf() -> Result<()> {
        let circuit_config = CircuitConfig::standard_recursion_config();
        let circuit = DummyLeafCircuit::new(&circuit_config);

        let zero_hash = HashOut::from([F::ZERO; 4]);
        let non_zero_hash = hash_str("Non-Zero Hash");

        let proof = circuit.prove(zero_hash)?;
        circuit.circuit.verify(proof)?;

        let proof = circuit.prove(non_zero_hash)?;
        circuit.circuit.verify(proof)?;

        Ok(())
    }

    #[test]
    fn verify_branch() -> Result<()> {
        let circuit_config = CircuitConfig::standard_recursion_config();
        let leaf_circuit = DummyLeafCircuit::new(&circuit_config);
        let branch_circuit_1 = DummyBranchCircuit::from_leaf(&circuit_config, &leaf_circuit);
        let branch_circuit_2 = DummyBranchCircuit::from_branch(&circuit_config, &branch_circuit_1);

        let zero_hash = HashOut::from([F::ZERO; 4]);
        let non_zero_hash_1 = hash_str("Non-Zero Hash 1");
        let non_zero_hash_2 = hash_str("Non-Zero Hash 2");
        let both_hash_1 = hash_branch(&non_zero_hash_1, &zero_hash);
        let both_hash_2 = hash_branch(&zero_hash, &non_zero_hash_2);
        let both_hash_1_2 = hash_branch(&both_hash_1, &both_hash_2);

        // Leaf proofs
        let zero_proof = leaf_circuit.prove(zero_hash)?;
        leaf_circuit.circuit.verify(zero_proof.clone())?;

        let non_zero_proof_1 = leaf_circuit.prove(non_zero_hash_1)?;
        leaf_circuit.circuit.verify(non_zero_proof_1.clone())?;

        let non_zero_proof_2 = leaf_circuit.prove(non_zero_hash_2)?;
        leaf_circuit.circuit.verify(non_zero_proof_2.clone())?;

        // Branch proofs
        let branch_1_and_0_proof =
            branch_circuit_1.prove(&non_zero_proof_1, &zero_proof, both_hash_1)?;
        branch_circuit_1
            .circuit
            .verify(branch_1_and_0_proof.clone())?;

        let branch_0_and_2_proof =
            branch_circuit_1.prove(&zero_proof, &non_zero_proof_2, both_hash_2)?;
        branch_circuit_1
            .circuit
            .verify(branch_0_and_2_proof.clone())?;

        // Double branch proofs
        let both1_2_branch_proof =
            branch_circuit_2.prove(&branch_1_and_0_proof, &branch_0_and_2_proof, both_hash_1_2)?;
        branch_circuit_2.circuit.verify(both1_2_branch_proof)?;

        Ok(())
    }
}

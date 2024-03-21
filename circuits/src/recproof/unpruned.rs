//! Subcircuits for recursively proving the entire contents of a merkle tree
//!
//! These subcircuits are pseudo-recursive, building on top of each other to
//! create the next level up of the merkle tree. "Pseudo-" here means the height
//! must be fixed ahead of time and not depend on the content.
//!
//! These subcircuits are useful because with just a pair of them, say a old and
//! new, you can prove a transition from the current merkle root (proved by old)
//! to a new merkle root (proved by new).
use itertools::chain;
use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::{HashOut, HashOutTarget, RichField, NUM_HASH_OUT_ELTS};
use plonky2::hash::poseidon2::Poseidon2Hash;
use plonky2::iop::target::Target;
use plonky2::iop::witness::{PartialWitness, WitnessWrite};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::proof::ProofWithPublicInputsTarget;

use super::{byte_wise_hash, find_hash};

/// The indices of the public inputs of this subcircuit in any
/// `ProofWithPublicInputs`
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
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

pub struct SubCircuitInputs {
    /// The hash of the unpruned state or ZERO if absent
    /// For leafs this is just an arbitrary values
    /// For branches this is the hash of `[left.unpruned_hash,
    /// right.unpruned_hash]`
    pub unpruned_hash: HashOutTarget,
}

pub struct LeafTargets {
    /// The public inputs
    pub inputs: SubCircuitInputs,
}

impl SubCircuitInputs {
    pub fn default<F, const D: usize>(builder: &mut CircuitBuilder<F, D>) -> Self
    where
        F: RichField + Extendable<D>, {
        let unpruned_hash = builder.add_virtual_hash();
        builder.register_public_inputs(&unpruned_hash.elements);
        Self { unpruned_hash }
    }

    #[must_use]
    pub fn build_leaf<F, const D: usize>(self, _builder: &mut CircuitBuilder<F, D>) -> LeafTargets
    where
        F: RichField + Extendable<D>, {
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
        let indices = PublicIndices {
            unpruned_hash: find_hash(public_inputs, self.inputs.unpruned_hash),
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
        unpruned_hash: HashOut<F>,
    ) {
        inputs.set_hash_target(self.targets.inputs.unpruned_hash, unpruned_hash);
    }
}

pub struct BranchTargets {
    /// The public inputs
    pub inputs: SubCircuitInputs,

    /// The left direction
    pub left: SubCircuitInputs,

    /// The right direction
    pub right: SubCircuitInputs,
}

impl SubCircuitInputs {
    #[must_use]
    pub fn build_branch<F: RichField + Extendable<D>, const D: usize>(
        self,
        builder: &mut CircuitBuilder<F, D>,
        indices: &PublicIndices,
        left_proof: &ProofWithPublicInputsTarget<D>,
        right_proof: &ProofWithPublicInputsTarget<D>,
        vm_hashing: bool,
    ) -> BranchTargets {
        let hasher = if vm_hashing {
            byte_wise_hash
        } else {
            CircuitBuilder::hash_n_to_hash_no_pad::<Poseidon2Hash>
        };

        let l_values = indices.get_unpruned_hash(&left_proof.public_inputs);
        let r_values = indices.get_unpruned_hash(&right_proof.public_inputs);

        // Hash the left and right together
        let unpruned_hash_calc = hasher(builder, chain!(l_values, r_values).collect());

        builder.connect_hashes(unpruned_hash_calc, self.unpruned_hash);

        BranchTargets {
            inputs: self,
            left: SubCircuitInputs {
                unpruned_hash: HashOutTarget::from(l_values),
            },
            right: SubCircuitInputs {
                unpruned_hash: HashOutTarget::from(r_values),
            },
        }
    }
}

/// The branch subcircuit metadata. This subcircuit proves knowledge of two
/// private subcircuit proofs, and that the public `unpruned_hash` values of
/// those circuits hash together to the public `unpruned_hash` value of this
/// circuit.
pub struct BranchSubCircuit {
    pub targets: BranchTargets,
    pub indices: PublicIndices,
}

impl BranchTargets {
    #[must_use]
    pub fn build(self, child: &PublicIndices, public_inputs: &[Target]) -> BranchSubCircuit {
        let indices = PublicIndices {
            unpruned_hash: find_hash(public_inputs, self.inputs.unpruned_hash),
        };
        debug_assert_eq!(indices, *child);

        BranchSubCircuit {
            indices,
            targets: self,
        }
    }
}

impl BranchSubCircuit {
    /// Get ready to generate a proof
    pub fn set_witness<F: RichField>(
        &self,
        inputs: &mut PartialWitness<F>,
        unpruned_hash: HashOut<F>,
    ) {
        inputs.set_hash_target(self.targets.inputs.unpruned_hash, unpruned_hash);
    }
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use itertools::Itertools;
    use lazy_static::lazy_static;
    use plonky2::field::types::Field;
    use plonky2::plonk::circuit_data::{CircuitConfig, CircuitData};
    use plonky2::plonk::config::Hasher;
    use plonky2::plonk::proof::ProofWithPublicInputs;

    use super::*;
    use crate::recproof::bounded;
    use crate::test_utils::{fast_test_circuit_config, hash_branch, hash_str, C, D, F};

    pub struct DummyLeafCircuit {
        pub bounded: bounded::LeafSubCircuit,
        pub unpruned: LeafSubCircuit,
        pub circuit: CircuitData<F, C, D>,
    }

    impl DummyLeafCircuit {
        #[must_use]
        pub fn new(circuit_config: &CircuitConfig) -> Self {
            let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());

            let bounded_inputs = bounded::SubCircuitInputs::default(&mut builder);
            let unpruned_inputs = SubCircuitInputs::default(&mut builder);

            let bounded_targets = bounded_inputs.build_leaf(&mut builder);
            let unpruned_targets = unpruned_inputs.build_leaf(&mut builder);

            let circuit = builder.build();

            let public_inputs = &circuit.prover_only.public_inputs;
            let bounded = bounded_targets.build(public_inputs);
            let unpruned = unpruned_targets.build(public_inputs);

            Self {
                bounded,
                unpruned,
                circuit,
            }
        }

        pub fn prove(&self, unpruned_hash: HashOut<F>) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.bounded.set_witness(&mut inputs);
            self.unpruned.set_witness(&mut inputs, unpruned_hash);
            self.circuit.prove(inputs)
        }
    }

    pub struct DummyBranchCircuit {
        pub bounded: bounded::BranchSubCircuit<D>,
        pub unpruned: BranchSubCircuit,
        pub circuit: CircuitData<F, C, D>,
    }

    impl DummyBranchCircuit {
        #[must_use]
        pub fn new(
            circuit_config: &CircuitConfig,
            indicies: &PublicIndices,
            child: &CircuitData<F, C, D>,
            vm_hash: bool,
        ) -> Self {
            let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());

            let bounded_inputs = bounded::SubCircuitInputs::default(&mut builder);
            let unpruned_inputs = SubCircuitInputs::default(&mut builder);

            let bounded_targets = bounded_inputs.build_branch(&mut builder, child);
            let unpruned_targets = unpruned_inputs.build_branch(
                &mut builder,
                indicies,
                &bounded_targets.left_proof,
                &bounded_targets.right_proof,
                vm_hash,
            );

            let circuit = builder.build();

            let public_inputs = &circuit.prover_only.public_inputs;
            let bounded = bounded_targets.build(public_inputs);
            let unpruned = unpruned_targets.build(indicies, public_inputs);

            Self {
                bounded,
                unpruned,
                circuit,
            }
        }

        #[must_use]
        pub fn from_leaf(
            circuit_config: &CircuitConfig,
            leaf: &DummyLeafCircuit,
            vm_hash: bool,
        ) -> Self {
            Self::new(
                circuit_config,
                &leaf.unpruned.indices,
                &leaf.circuit,
                vm_hash,
            )
        }

        #[must_use]
        pub fn from_branch(circuit_config: &CircuitConfig, branch: &Self, vm_hash: bool) -> Self {
            Self::new(
                circuit_config,
                &branch.unpruned.indices,
                &branch.circuit,
                vm_hash,
            )
        }

        pub fn prove(
            &self,
            unpruned_hash: HashOut<F>,
            left_proof: &ProofWithPublicInputs<F, C, D>,
            right_proof: &ProofWithPublicInputs<F, C, D>,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.bounded
                .set_witness(&mut inputs, left_proof, right_proof);
            self.unpruned.set_witness(&mut inputs, unpruned_hash);
            self.circuit.prove(inputs)
        }
    }

    const CONFIG: CircuitConfig = fast_test_circuit_config();

    lazy_static! {
        static ref LEAF: DummyLeafCircuit = DummyLeafCircuit::new(&CONFIG);
        static ref BRANCH_1: DummyBranchCircuit =
            DummyBranchCircuit::from_leaf(&CONFIG, &LEAF, false);
        static ref BRANCH_2: DummyBranchCircuit =
            DummyBranchCircuit::from_branch(&CONFIG, &BRANCH_1, false);
        static ref VM_BRANCH_1: DummyBranchCircuit =
            DummyBranchCircuit::from_leaf(&CONFIG, &LEAF, true);
        static ref VM_BRANCH_2: DummyBranchCircuit =
            DummyBranchCircuit::from_branch(&CONFIG, &VM_BRANCH_1, true);
    }

    #[test]
    fn verify_leaf() -> Result<()> {
        let zero_hash = HashOut::from([F::ZERO; 4]);
        let non_zero_hash = hash_str("Non-Zero Hash");

        let proof = LEAF.prove(zero_hash)?;
        LEAF.circuit.verify(proof)?;

        let proof = LEAF.prove(non_zero_hash)?;
        LEAF.circuit.verify(proof)?;

        Ok(())
    }

    #[test]
    fn verify_branch() -> Result<()> {
        let zero_hash = HashOut::from([F::ZERO; 4]);
        let non_zero_hash_1 = hash_str("Non-Zero Hash 1");
        let non_zero_hash_2 = hash_str("Non-Zero Hash 2");
        let both_hash_1 = hash_branch(&non_zero_hash_1, &zero_hash);
        let both_hash_2 = hash_branch(&zero_hash, &non_zero_hash_2);
        let both_hash_1_2 = hash_branch(&both_hash_1, &both_hash_2);

        // Leaf proofs
        let zero_proof = LEAF.prove(zero_hash)?;
        LEAF.circuit.verify(zero_proof.clone())?;

        let non_zero_proof_1 = LEAF.prove(non_zero_hash_1)?;
        LEAF.circuit.verify(non_zero_proof_1.clone())?;

        let non_zero_proof_2 = LEAF.prove(non_zero_hash_2)?;
        LEAF.circuit.verify(non_zero_proof_2.clone())?;

        // Branch proofs
        let branch_1_and_0_proof = BRANCH_1.prove(both_hash_1, &non_zero_proof_1, &zero_proof)?;
        BRANCH_1.circuit.verify(branch_1_and_0_proof.clone())?;

        let branch_0_and_2_proof = BRANCH_1.prove(both_hash_2, &zero_proof, &non_zero_proof_2)?;
        BRANCH_1.circuit.verify(branch_0_and_2_proof.clone())?;

        // Double branch proofs
        let both1_2_branch_proof =
            BRANCH_2.prove(both_hash_1_2, &branch_1_and_0_proof, &branch_0_and_2_proof)?;
        BRANCH_2.circuit.verify(both1_2_branch_proof)?;

        Ok(())
    }

    fn hash_branch_bytes<F: RichField>(left: &HashOut<F>, right: &HashOut<F>) -> HashOut<F> {
        let bytes = chain!(left.elements, right.elements)
            .flat_map(|v| v.to_canonical_u64().to_le_bytes())
            .map(|v| F::from_canonical_u8(v))
            .collect_vec();
        Poseidon2Hash::hash_no_pad(&bytes)
    }

    #[test]
    fn verify_branch_bytes() -> Result<()> {
        let zero_hash = HashOut::from([F::ZERO; 4]);
        let non_zero_hash_1 = hash_str("Non-Zero Hash 1");
        let non_zero_hash_2 = hash_str("Non-Zero Hash 2");
        let both_hash_1 = hash_branch_bytes(&non_zero_hash_1, &zero_hash);
        let both_hash_2 = hash_branch_bytes(&zero_hash, &non_zero_hash_2);
        let both_hash_1_2 = hash_branch_bytes(&both_hash_1, &both_hash_2);

        // Leaf proofs
        let zero_proof = LEAF.prove(zero_hash)?;
        LEAF.circuit.verify(zero_proof.clone())?;

        let non_zero_proof_1 = LEAF.prove(non_zero_hash_1)?;
        LEAF.circuit.verify(non_zero_proof_1.clone())?;

        let non_zero_proof_2 = LEAF.prove(non_zero_hash_2)?;
        LEAF.circuit.verify(non_zero_proof_2.clone())?;

        // Branch proofs
        let branch_1_and_0_proof =
            VM_BRANCH_1.prove(both_hash_1, &non_zero_proof_1, &zero_proof)?;
        VM_BRANCH_1.circuit.verify(branch_1_and_0_proof.clone())?;

        let branch_0_and_2_proof =
            VM_BRANCH_1.prove(both_hash_2, &zero_proof, &non_zero_proof_2)?;
        VM_BRANCH_1.circuit.verify(branch_0_and_2_proof.clone())?;

        // Double branch proofs
        let both1_2_branch_proof =
            VM_BRANCH_2.prove(both_hash_1_2, &branch_1_and_0_proof, &branch_0_and_2_proof)?;
        VM_BRANCH_2.circuit.verify(both1_2_branch_proof)?;

        Ok(())
    }
}

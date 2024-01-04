use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::{HashOut, HashOutTarget, RichField, NUM_HASH_OUT_ELTS};
use plonky2::hash::poseidon2::Poseidon2Hash;
use plonky2::iop::witness::{PartialWitness, WitnessWrite};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::CircuitData;
use plonky2::plonk::config::GenericConfig;
use plonky2::plonk::proof::ProofWithPublicInputsTarget;

use super::SubCircuit;

#[derive(Copy, Clone)]
pub struct PublicIndices {
    pub unpruned_hash: [usize; NUM_HASH_OUT_ELTS],
}

impl PublicIndices {
    pub fn get_unpruned_hash<T: Copy>(&self, public_inputs: &[T]) -> [T; NUM_HASH_OUT_ELTS] {
        self.unpruned_hash.map(|i| public_inputs[i])
    }

    pub fn set_unpruned_hash<T>(&self, public_inputs: &mut [T], v: [T; NUM_HASH_OUT_ELTS]) {
        for (i, v) in v.into_iter().enumerate() {
            public_inputs[self.unpruned_hash[i]] = v;
        }
    }
}

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
    pub fn new<F, C, const D: usize, T, B, R>(
        mut builder: CircuitBuilder<F, D>,
        t: T,
        build: B,
    ) -> (CircuitData<F, C, D>, (Self, R))
    where
        B: FnOnce(T, &LeafTargets, CircuitBuilder<F, D>) -> (CircuitData<F, C, D>, R),
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F>, {
        let unpruned_hash = builder.add_virtual_hash();
        builder.register_public_inputs(&unpruned_hash.elements);

        let targets = LeafTargets { unpruned_hash };
        let (circuit, r) = build(t, &targets, builder);

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

    pub fn set_inputs<F: RichField>(
        &self,
        inputs: &mut PartialWitness<F>,
        unpruned_hash: HashOut<F>,
    ) {
        inputs.set_hash_target(self.targets.unpruned_hash, unpruned_hash);
    }
}

impl SubCircuit<PublicIndices> for LeafSubCircuit {
    fn pis(&self) -> usize { 4 }

    fn get_indices(&self) -> PublicIndices { self.indices }
}

pub struct BranchSubCircuit<'a, const D: usize> {
    pub targets: BranchTargets<D>,
    pub indices: PublicIndices,
    /// The distance from the leaves (`0`` being the lowest branch)
    /// Used for debugging
    pub height: usize,
    pub inner_circuit: &'a dyn SubCircuit<PublicIndices>,
}

pub struct BranchTargets<const D: usize> {
    /// The left dir
    pub left_dir: BranchDirTargets<D>,

    /// The right dir
    pub right_dir: BranchDirTargets<D>,

    /// The hash of `[left.unpruned_hash, right.unpruned_hash]`
    pub unpruned_hash: HashOutTarget,
}

pub struct BranchDirTargets<const D: usize> {
    /// The hash of this dir proved by `proof`
    pub unpruned_hash: HashOutTarget,
}

impl<'a, const D: usize> BranchSubCircuit<'a, D> {
    fn from_dirs<F, C, B, R>(
        inner_circuit: &'a dyn SubCircuit<PublicIndices>,
        mut builder: CircuitBuilder<F, D>,
        left_dir: BranchDirTargets<D>,
        right_dir: BranchDirTargets<D>,
        height: usize,
        build: B,
    ) -> (CircuitData<F, C, D>, (Self, R))
    where
        B: FnOnce(CircuitBuilder<F, D>) -> (CircuitData<F, C, D>, R),
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F>, {
        let unpruned_hash = builder.hash_n_to_hash_no_pad::<Poseidon2Hash>(
            left_dir
                .unpruned_hash
                .elements
                .into_iter()
                .chain(right_dir.unpruned_hash.elements)
                .collect(),
        );

        builder.register_public_inputs(&unpruned_hash.elements);

        let (circuit, r) = build(builder);

        let targets = BranchTargets {
            left_dir,
            right_dir,
            unpruned_hash,
        };
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
            inner_circuit,
        };

        (circuit, (v, r))
    }

    fn dir_from_node(
        proof: &ProofWithPublicInputsTarget<D>,
        sub_circuit: &dyn SubCircuit<PublicIndices>,
    ) -> BranchDirTargets<D> {
        let node_idx = sub_circuit.get_indices();

        let unpruned_hash = HashOutTarget::from(node_idx.get_unpruned_hash(&proof.public_inputs));

        BranchDirTargets { unpruned_hash }
    }

    pub fn from_leaf<F, C, B, R>(
        builder: CircuitBuilder<F, D>,
        leaf: &'a LeafSubCircuit,
        left_proof: &ProofWithPublicInputsTarget<D>,
        right_proof: &ProofWithPublicInputsTarget<D>,
        build: B,
    ) -> (CircuitData<F, C, D>, (Self, R))
    where
        B: FnOnce(CircuitBuilder<F, D>) -> (CircuitData<F, C, D>, R),
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F>, {
        let left_dir = Self::dir_from_node(left_proof, leaf);
        let right_dir = Self::dir_from_node(right_proof, leaf);
        let height = 0;
        Self::from_dirs(leaf, builder, left_dir, right_dir, height, build)
    }

    pub fn from_branch<F, C, B, R>(
        builder: CircuitBuilder<F, D>,
        branch: &'a BranchSubCircuit<'a, D>,
        left_proof: &ProofWithPublicInputsTarget<D>,
        right_proof: &ProofWithPublicInputsTarget<D>,
        build: B,
    ) -> (CircuitData<F, C, D>, (Self, R))
    where
        B: FnOnce(CircuitBuilder<F, D>) -> (CircuitData<F, C, D>, R),
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F>, {
        let left_dir = Self::dir_from_node(left_proof, branch);
        let right_dir = Self::dir_from_node(right_proof, branch);
        let height = branch.height + 1;
        Self::from_dirs(branch, builder, left_dir, right_dir, height, build)
    }

    pub fn set_inputs<F: RichField>(
        &self,
        inputs: &mut PartialWitness<F>,
        unpruned_hash: HashOut<F>,
    ) {
        inputs.set_hash_target(self.targets.unpruned_hash, unpruned_hash);
    }
}

impl<'a, const D: usize> SubCircuit<PublicIndices> for BranchSubCircuit<'a, D> {
    fn pis(&self) -> usize { 4 }

    fn get_indices(&self) -> PublicIndices { self.indices }
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use plonky2::field::types::Field;
    use plonky2::plonk::circuit_data::CircuitConfig;
    use plonky2::plonk::proof::ProofWithPublicInputs;

    use super::*;
    use crate::recproof::test::{hash_branch, hash_str};
    use crate::test_utils::{C, D, F};

    pub struct DummyLeafCircuit {
        pub unpruned: LeafSubCircuit,
        pub circuit: CircuitData<F, C, D>,
    }

    impl DummyLeafCircuit {
        #[must_use]
        pub fn new(circuit_config: &CircuitConfig) -> Self {
            let builder = CircuitBuilder::<F, D>::new(circuit_config.clone());
            let (circuit, (unpruned, ())) =
                LeafSubCircuit::new(builder, (), |(), _targets, builder| (builder.build(), ()));

            Self { unpruned, circuit }
        }

        pub fn prove(&self, unpruned_hash: HashOut<F>) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.unpruned.set_inputs(&mut inputs, unpruned_hash);
            self.circuit.prove(inputs)
        }
    }

    pub struct DummyBranchCircuit<'a> {
        pub summarized: BranchSubCircuit<'a, D>,
        pub circuit: CircuitData<F, C, D>,
        pub targets: DummyBranchTargets,
    }

    pub struct DummyBranchTargets {
        pub left_proof: ProofWithPublicInputsTarget<D>,
        pub right_proof: ProofWithPublicInputsTarget<D>,
    }

    impl<'a> DummyBranchCircuit<'a> {
        #[must_use]
        pub fn from_leaf(circuit_config: &CircuitConfig, leaf: &'a DummyLeafCircuit) -> Self {
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
                |builder| (builder.build(), ()),
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

        pub fn from_branch(circuit_config: &CircuitConfig, branch: &'a DummyBranchCircuit) -> Self {
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
                |builder| (builder.build(), ()),
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
        let both1_branch_proof =
            branch_circuit_1.prove(&non_zero_proof_1, &zero_proof, both_hash_1)?;
        branch_circuit_1
            .circuit
            .verify(both1_branch_proof.clone())?;

        let both2_branch_proof =
            branch_circuit_1.prove(&zero_proof, &non_zero_proof_2, both_hash_2)?;
        branch_circuit_1
            .circuit
            .verify(both2_branch_proof.clone())?;

        // Double branch proofs
        let both1_2_branch_proof =
            branch_circuit_2.prove(&both1_branch_proof, &both2_branch_proof, both_hash_1_2)?;
        branch_circuit_2.circuit.verify(both1_2_branch_proof)?;

        Ok(())
    }
}

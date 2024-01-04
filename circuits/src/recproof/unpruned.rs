use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::{HashOut, HashOutTarget, RichField, NUM_HASH_OUT_ELTS};
use plonky2::hash::poseidon2::Poseidon2Hash;
use plonky2::iop::witness::{PartialWitness, WitnessWrite};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::{CircuitData, VerifierCircuitTarget};
use plonky2::plonk::config::{AlgebraicHasher, GenericConfig};
use plonky2::plonk::proof::ProofWithPublicInputsTarget;

use super::{BranchCircuit, Circuit, CircuitType, LeafCircuit, SubCircuit};

pub struct Type;

impl CircuitType for Type {
    type BranchSubCircuit<'a, const D: usize> = BranchSubCircuit<'a, D>;
    type LeafSubCircuit = LeafSubCircuit;
    type PublicIndices = PublicIndices;
}

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
    pub fn new<F, C, const D: usize, B, R>(
        mut builder: CircuitBuilder<F, D>,
        build: B,
    ) -> (CircuitData<F, C, D>, (Self, R))
    where
        B: FnOnce(CircuitBuilder<F, D>) -> (CircuitData<F, C, D>, R),
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F>, {
        let unpruned_hash = builder.add_virtual_hash();
        builder.register_public_inputs(&unpruned_hash.elements);

        let (circuit, r) = build(builder);

        let targets = LeafTargets { unpruned_hash };
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

    /// The proof of this branch
    pub proof: ProofWithPublicInputsTarget<D>,
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

    fn dir_from_node<F, C>(
        builder: &mut CircuitBuilder<F, D>,
        verifier: &VerifierCircuitTarget,
        node: &dyn Circuit<Type, F, C, D>,
    ) -> BranchDirTargets<D>
    where
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F>,
        <C as GenericConfig<D>>::Hasher: AlgebraicHasher<F>, {
        let common = &node.circuit_data().common;
        let proof = builder.add_virtual_proof_with_pis(common);
        let node_idx = node.sub_circuit().get_indices();

        let unpruned_hash = HashOutTarget::from(node_idx.get_unpruned_hash(&proof.public_inputs));

        builder.verify_proof::<C>(&proof, verifier, common);

        BranchDirTargets {
            unpruned_hash,
            proof,
        }
    }

    pub fn from_leaf<F, C, B, R>(
        mut builder: CircuitBuilder<F, D>,
        leaf: &'a dyn LeafCircuit<Type, F, C, D>,
        build: B,
    ) -> (CircuitData<F, C, D>, (Self, R))
    where
        B: FnOnce(CircuitBuilder<F, D>) -> (CircuitData<F, C, D>, R),
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F>,
        <C as GenericConfig<D>>::Hasher: AlgebraicHasher<F>, {
        let verifier = builder.constant_verifier_data(&leaf.circuit_data().verifier_only);
        let left_dir = Self::dir_from_node(&mut builder, &verifier, leaf);
        let right_dir = Self::dir_from_node(&mut builder, &verifier, leaf);
        let height = 0;
        Self::from_dirs(
            leaf.sub_circuit(),
            builder,
            left_dir,
            right_dir,
            height,
            build,
        )
    }

    pub fn from_branch<F, C, B, R>(
        mut builder: CircuitBuilder<F, D>,
        branch: &'a dyn BranchCircuit<Type, F, C, D>,
        build: B,
    ) -> (CircuitData<F, C, D>, (Self, R))
    where
        B: FnOnce(CircuitBuilder<F, D>) -> (CircuitData<F, C, D>, R),
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F>,
        <C as GenericConfig<D>>::Hasher: AlgebraicHasher<F>, {
        let verifier = builder.constant_verifier_data(&branch.circuit_data().verifier_only);
        let left_dir = Self::dir_from_node(&mut builder, &verifier, branch);
        let right_dir = Self::dir_from_node(&mut builder, &verifier, branch);
        let height = branch.branch_sub_circuit().height + 1;
        Self::from_dirs(
            branch.sub_circuit(),
            builder,
            left_dir,
            right_dir,
            height,
            build,
        )
    }

    // pub fn set_inputs<F: RichField>(
    //     &self,
    //     inputs: &mut PartialWitness<F>,
    //     unpruned_hash: HashOut<F>,
    // ) {
    //     inputs.set_hash_target(self.targets.unpruned_hash, unpruned_hash);
    // }
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
    use plonky2::plonk::config::Hasher;
    use plonky2::plonk::proof::ProofWithPublicInputs;

    use super::*;
    use crate::test_utils::{C, D, F};

    struct TestCircuit {
        pub unpruned: LeafSubCircuit,
        pub circuit: CircuitData<F, C, D>,
    }

    impl TestCircuit {
        #[must_use]
        pub fn new(circuit_config: &CircuitConfig) -> Self {
            let builder = CircuitBuilder::<F, D>::new(circuit_config.clone());
            let (circuit, (unpruned, ())) =
                LeafSubCircuit::new(builder, |builder| (builder.build(), ()));

            Self { unpruned, circuit }
        }

        pub fn prove(&self, unpruned_hash: HashOut<F>) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.unpruned.set_inputs(&mut inputs, unpruned_hash);
            self.circuit.prove(inputs)
        }
    }

    fn hash_str(v: &str) -> HashOut<F> {
        let v: Vec<_> = v.bytes().map(F::from_canonical_u8).collect();
        Poseidon2Hash::hash_no_pad(&v)
    }

    #[test]
    fn verify_leaf() -> Result<()> {
        let circuit_config = CircuitConfig::standard_recursion_config();
        let circuit = TestCircuit::new(&circuit_config);

        let zero_hash = HashOut::from([F::ZERO; 4]);
        let non_zero_hash = hash_str("Non-Zero Hash");

        let proof = circuit.prove(zero_hash)?;
        circuit.circuit.verify(proof)?;

        let proof = circuit.prove(non_zero_hash)?;
        circuit.circuit.verify(proof)?;

        Ok(())
    }
}

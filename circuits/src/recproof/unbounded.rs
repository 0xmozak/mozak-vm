//! Subcircuits for recursively proving an unbounded binary tree.
//!
//! These subcircuits are fully-recursive, building on top of each other to
//! create the next level up of the tree.
use std::iter::zip;

use plonky2::field::extension::Extendable;
use plonky2::gates::noop::NoopGate;
use plonky2::hash::hash_types::{MerkleCapTarget, RichField, NUM_HASH_OUT_ELTS};
use plonky2::iop::target::{BoolTarget, Target};
use plonky2::iop::witness::{PartialWitness, WitnessWrite};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::{CircuitData, VerifierCircuitTarget};
use plonky2::plonk::config::{AlgebraicHasher, GenericConfig};
use plonky2::plonk::proof::{ProofWithPublicInputs, ProofWithPublicInputsTarget};

use super::{find_hash, select_verifier};
use crate::stark::recursive_verifier::circuit_data_for_recursion;

/// Plonky2's recursion threshold is 2^12 gates. We use a slightly relaxed
/// threshold here to support the case that two proofs are verified in the same
/// circuit.
const RECPROOF_RECURSION_THRESHOLD_DEGREE_BITS: usize = 13;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PublicIndices {
    /// A digest of the "circuit" (i.e. the instance, minus public inputs),
    /// which can be used to seed Fiat-Shamir.
    pub circuit_digest: [usize; NUM_HASH_OUT_ELTS],
    /// A commitment to each constant polynomial and each permutation
    /// polynomial.
    pub constants_sigmas_cap: Vec<[usize; NUM_HASH_OUT_ELTS]>,
}

impl PublicIndices {
    /// Extract `circuit_digest` from an array of public inputs.
    pub fn get_circuit_digest<T: Copy>(&self, public_inputs: &[T]) -> [T; NUM_HASH_OUT_ELTS] {
        self.circuit_digest.map(|i| public_inputs[i])
    }

    /// Insert `circuit_digest` into an array of public inputs.
    pub fn set_circuit_digest<T>(&self, public_inputs: &mut [T], v: [T; NUM_HASH_OUT_ELTS]) {
        for (i, v) in v.into_iter().enumerate() {
            public_inputs[self.circuit_digest[i]] = v;
        }
    }

    /// Extract `constants_sigmas_cap` from an array of public inputs.
    pub fn get_constants_sigmas_cap<T: Copy>(
        &self,
        public_inputs: &[T],
    ) -> Vec<[T; NUM_HASH_OUT_ELTS]> {
        self.constants_sigmas_cap
            .iter()
            .map(|v| v.map(|i| public_inputs[i]))
            .collect()
    }

    /// Insert `constants_sigmas_cap` into an array of public inputs.
    pub fn set_constants_sigmas_cap<T>(
        &self,
        public_inputs: &mut [T],
        vs: Vec<[T; NUM_HASH_OUT_ELTS]>,
    ) {
        for (i, v) in vs.into_iter().enumerate() {
            for (i, v) in zip(self.constants_sigmas_cap[i], v) {
                public_inputs[i] = v;
            }
        }
    }
}

pub struct SubCircuitInputs {
    pub verifier: VerifierCircuitTarget,
}

pub struct LeafTargets {
    /// The public inputs
    pub inputs: SubCircuitInputs,
}

impl SubCircuitInputs {
    pub fn default<F, const D: usize>(builder: &mut CircuitBuilder<F, D>) -> Self
    where
        F: RichField + Extendable<D>, {
        let verifier = builder.add_virtual_verifier_data(builder.config.fri_config.cap_height);
        builder.register_public_inputs(&verifier.circuit_digest.elements);
        for i in 0..builder.config.fri_config.num_cap_elements() {
            builder.register_public_inputs(&verifier.constants_sigmas_cap.0[i].elements);
        }
        Self { verifier }
    }

    #[must_use]
    pub fn build_leaf<F, C, const D: usize>(
        self,
        builder: &mut CircuitBuilder<F, D>,
    ) -> LeafTargets
    where
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F>,
        C::Hasher: AlgebraicHasher<F>, {
        let common_data = circuit_data_for_recursion::<F, C, D>(
            &builder.config,
            RECPROOF_RECURSION_THRESHOLD_DEGREE_BITS,
            0,
        )
        .common;

        // common_data.num_public_inputs = builder.num_public_inputs();

        // Make sure we have enough gates to match `common_data`.
        while builder.num_gates() < (common_data.degree() / 2) {
            builder.add_gate(NoopGate, vec![]);
        }
        // Make sure we have every gate to match `common_data`.
        for g in &common_data.gates {
            builder.add_gate_to_gate_set(g.clone());
        }

        LeafTargets { inputs: self }
    }
}

/// The leaf subcircuit metadata. This subcircuit does basically nothing, simply
/// expressing a verifier to bind the tree's recursion
pub struct LeafSubCircuit {
    pub targets: LeafTargets,
    pub indices: PublicIndices,
}

impl LeafTargets {
    #[must_use]
    pub fn build(self, public_inputs: &[Target]) -> LeafSubCircuit {
        let indices = PublicIndices {
            circuit_digest: find_hash(public_inputs, self.inputs.verifier.circuit_digest),
            constants_sigmas_cap: self
                .inputs
                .verifier
                .constants_sigmas_cap
                .0
                .iter()
                .map(|v| find_hash(public_inputs, *v))
                .collect(),
        };
        LeafSubCircuit {
            targets: self,
            indices,
        }
    }
}

impl LeafSubCircuit {
    /// Get ready to generate a proof
    pub fn set_witness<F, C, const D: usize>(
        &self,
        inputs: &mut PartialWitness<F>,
        branch: &CircuitData<F, C, D>,
    ) where
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F>,
        C::Hasher: AlgebraicHasher<F>, {
        inputs.set_verifier_data_target(&self.targets.inputs.verifier, &branch.verifier_only);
    }
}

pub struct BranchTargets<const D: usize> {
    /// The public inputs
    pub inputs: SubCircuitInputs,

    /// Indicates if the left branch is a leaf or not
    pub left_is_leaf: BoolTarget,

    /// Indicates if the right branch is a leaf or not
    pub right_is_leaf: BoolTarget,

    /// The left proof
    pub left_proof: ProofWithPublicInputsTarget<D>,
    /// The right proof
    pub right_proof: ProofWithPublicInputsTarget<D>,
}

impl SubCircuitInputs {
    #[must_use]
    pub fn build_branch<F, C, const D: usize>(
        self,
        builder: &mut CircuitBuilder<F, D>,
        leaf: &LeafSubCircuit,
        circuit: &CircuitData<F, C, D>,
    ) -> BranchTargets<D>
    where
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F>,
        C::Hasher: AlgebraicHasher<F>, {
        let common = &circuit.common;
        let left_is_leaf = builder.add_virtual_bool_target_safe();
        let right_is_leaf = builder.add_virtual_bool_target_safe();
        let left_proof = builder.add_virtual_proof_with_pis(common);
        let right_proof = builder.add_virtual_proof_with_pis(common);
        let leaf_verifier = builder.constant_verifier_data(&circuit.verifier_only);

        // Connect previous verifier data to current one. This guarantees that every
        // proof in the cycle uses the same verifier data.
        let left_verifier = VerifierCircuitTarget {
            circuit_digest: leaf
                .indices
                .get_circuit_digest(&left_proof.public_inputs)
                .into(),
            constants_sigmas_cap: MerkleCapTarget(
                leaf.indices
                    .get_constants_sigmas_cap(&left_proof.public_inputs)
                    .into_iter()
                    .map(Into::into)
                    .collect(),
            ),
        };
        let right_verifier = VerifierCircuitTarget {
            circuit_digest: leaf
                .indices
                .get_circuit_digest(&right_proof.public_inputs)
                .into(),
            constants_sigmas_cap: MerkleCapTarget(
                leaf.indices
                    .get_constants_sigmas_cap(&right_proof.public_inputs)
                    .into_iter()
                    .map(Into::into)
                    .collect(),
            ),
        };
        builder.connect_verifier_data(&self.verifier, &left_verifier);
        builder.connect_verifier_data(&self.verifier, &right_verifier);

        let left_verifier = select_verifier(builder, left_is_leaf, &leaf_verifier, &self.verifier);
        let right_verifier =
            select_verifier(builder, right_is_leaf, &leaf_verifier, &self.verifier);
        builder.verify_proof::<C>(&left_proof, &left_verifier, common);
        builder.verify_proof::<C>(&right_proof, &right_verifier, common);

        // Make sure we have enough gates to match `common_data`.
        while builder.num_gates() < (common.degree() / 2) {
            builder.add_gate(NoopGate, vec![]);
        }

        // Make sure we have every gate to match `common_data`.
        for g in &common.gates {
            builder.add_gate_to_gate_set(g.clone());
        }

        BranchTargets {
            inputs: self,
            left_is_leaf,
            right_is_leaf,
            left_proof,
            right_proof,
        }
    }
}

/// The branch subcircuit metadata. This subcircuit proves knowledge of two
/// private subcircuit proofs, all bound to use the same recursive verifier.
pub struct BranchSubCircuit<const D: usize> {
    pub targets: BranchTargets<D>,
    pub indices: PublicIndices,
}

impl<const D: usize> BranchTargets<D> {
    #[must_use]
    pub fn build(self, leaf: &LeafSubCircuit, public_inputs: &[Target]) -> BranchSubCircuit<D> {
        // Find the indicies
        let indices = PublicIndices {
            circuit_digest: find_hash(public_inputs, self.inputs.verifier.circuit_digest),
            constants_sigmas_cap: self
                .inputs
                .verifier
                .constants_sigmas_cap
                .0
                .iter()
                .map(|v| find_hash(public_inputs, *v))
                .collect(),
        };
        debug_assert_eq!(indices, leaf.indices);

        BranchSubCircuit {
            targets: self,
            indices,
        }
    }
}

impl<const D: usize> BranchSubCircuit<D> {
    pub fn set_witness<F, C>(
        &self,
        inputs: &mut PartialWitness<F>,
        left_is_leaf: bool,
        right_is_leaf: bool,
        left_proof: &ProofWithPublicInputs<F, C, D>,
        right_proof: &ProofWithPublicInputs<F, C, D>,
    ) where
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F>,
        <C as GenericConfig<D>>::Hasher: AlgebraicHasher<F>, {
        inputs.set_bool_target(self.targets.left_is_leaf, left_is_leaf);
        inputs.set_bool_target(self.targets.right_is_leaf, right_is_leaf);
        inputs.set_proof_with_pis_target(&self.targets.left_proof, left_proof);
        inputs.set_proof_with_pis_target(&self.targets.right_proof, right_proof);
    }

    pub fn set_partial_witness<F, C>(
        &self,
        inputs: &mut PartialWitness<F>,
        left_proof: &ProofWithPublicInputs<F, C, D>,
        right_proof: &ProofWithPublicInputs<F, C, D>,
    ) where
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F>,
        <C as GenericConfig<D>>::Hasher: AlgebraicHasher<F>, {
        inputs.set_proof_with_pis_target(&self.targets.left_proof, left_proof);
        inputs.set_proof_with_pis_target(&self.targets.right_proof, right_proof);
    }
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use lazy_static::lazy_static;
    use plonky2::plonk::circuit_data::CircuitConfig;
    use plonky2::plonk::proof::ProofWithPublicInputs;

    use super::*;
    use crate::test_utils::{fast_test_circuit_config, C, D, F};

    pub struct DummyLeafCircuit {
        pub unbounded: LeafSubCircuit,
        pub circuit: CircuitData<F, C, D>,
    }

    impl DummyLeafCircuit {
        #[must_use]
        pub fn new(circuit_config: &CircuitConfig) -> Self {
            let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());
            let unbounded_inputs = SubCircuitInputs::default(&mut builder);

            let unbounded_targets = unbounded_inputs.build_leaf::<F, C, D>(&mut builder);

            let circuit = builder.build();

            let unbounded = unbounded_targets.build(&circuit.prover_only.public_inputs);

            Self { unbounded, circuit }
        }

        pub fn prove(&self, branch: &DummyBranchCircuit) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.unbounded.set_witness(&mut inputs, &branch.circuit);
            self.circuit.prove(inputs)
        }
    }

    pub struct DummyBranchCircuit {
        pub unbounded: BranchSubCircuit<D>,
        pub circuit: CircuitData<F, C, D>,
    }

    impl DummyBranchCircuit {
        #[must_use]
        pub fn new(circuit_config: &CircuitConfig, leaf: &DummyLeafCircuit) -> Self {
            let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());
            let unbounded_inputs = SubCircuitInputs::default(&mut builder);

            let unbounded_targets =
                unbounded_inputs.build_branch(&mut builder, &leaf.unbounded, &leaf.circuit);

            let circuit = builder.build();

            let unbounded =
                unbounded_targets.build(&leaf.unbounded, &circuit.prover_only.public_inputs);

            Self { unbounded, circuit }
        }

        pub fn prove(
            &self,
            left_is_leaf: bool,
            right_is_leaf: bool,
            left_proof: &ProofWithPublicInputs<F, C, D>,
            right_proof: &ProofWithPublicInputs<F, C, D>,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.unbounded.set_witness(
                &mut inputs,
                left_is_leaf,
                right_is_leaf,
                left_proof,
                right_proof,
            );
            self.circuit.prove(inputs)
        }
    }

    const CONFIG: CircuitConfig = fast_test_circuit_config();

    lazy_static! {
        static ref LEAF: DummyLeafCircuit = DummyLeafCircuit::new(&CONFIG);
        static ref BRANCH: DummyBranchCircuit = DummyBranchCircuit::new(&CONFIG, &LEAF);
    }

    #[test]
    fn verify_leaf() -> Result<()> {
        let proof = LEAF.prove(&BRANCH)?;
        LEAF.circuit.verify(proof)?;

        Ok(())
    }

    #[test]
    fn verify_branch() -> Result<()> {
        let leaf_proof = LEAF.prove(&BRANCH)?;
        LEAF.circuit.verify(leaf_proof.clone())?;

        let branch_proof_1 = BRANCH.prove(true, true, &leaf_proof, &leaf_proof)?;
        BRANCH.circuit.verify(branch_proof_1.clone())?;

        let branch_proof_2 = BRANCH.prove(true, false, &leaf_proof, &branch_proof_1)?;
        BRANCH.circuit.verify(branch_proof_2.clone())?;

        let branch_proof_3 = BRANCH.prove(false, false, &branch_proof_1, &branch_proof_2)?;
        BRANCH.circuit.verify(branch_proof_3)?;

        Ok(())
    }
}

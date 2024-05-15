//! Subcircuits embedding the merge of two binary merkle trees.
//!
//! This lets you do the merge externally in however many recursive steps and
//! just embed the proof of the final merge in another circuit.

use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::{HashOut, HashOutTarget, RichField};
use plonky2::iop::target::{BoolTarget, Target};
use plonky2::iop::witness::{PartialWitness, WitnessWrite};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::config::{AlgebraicHasher, GenericConfig};
use plonky2::plonk::proof::{ProofWithPublicInputs, ProofWithPublicInputsTarget};

use crate::indices::{BoolTargetIndex, HashOutTargetIndex};
use crate::{false_if, zero_hash_if};

/// The indices of the public inputs of this subcircuit in any
/// `ProofWithPublicInputs`
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct PublicIndices {
    /// The index for the presence of the hash
    pub hash_present: BoolTargetIndex,

    /// The indices of each of the elements of the hash
    pub hash: HashOutTargetIndex,
}

pub struct SubCircuitInputs {
    /// The presence of `hash`
    pub hash_present: BoolTarget,

    /// The hash or ZERO if absent
    pub hash: HashOutTarget,
}

pub struct LeafTargets {
    /// The public inputs
    pub inputs: SubCircuitInputs,
}

impl SubCircuitInputs {
    pub fn default<F, const D: usize>(builder: &mut CircuitBuilder<F, D>) -> Self
    where
        F: RichField + Extendable<D>, {
        let hash_present = builder.add_virtual_bool_target_safe();
        let hash = builder.add_virtual_hash();
        builder.register_public_input(hash_present.target);
        builder.register_public_inputs(&hash.elements);

        Self { hash_present, hash }
    }

    #[must_use]
    pub fn build_leaf<F, const D: usize>(self, _builder: &mut CircuitBuilder<F, D>) -> LeafTargets
    where
        F: RichField + Extendable<D>, {
        LeafTargets { inputs: self }
    }
}

pub struct LeafSubCircuit {
    pub targets: LeafTargets,
    pub indices: PublicIndices,
}

impl LeafTargets {
    #[must_use]
    pub fn build(self, public_inputs: &[Target]) -> LeafSubCircuit {
        // Find the indices
        let indices = PublicIndices {
            hash_present: BoolTargetIndex::new(public_inputs, self.inputs.hash_present),
            hash: HashOutTargetIndex::new(public_inputs, self.inputs.hash),
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
        hash: Option<HashOut<F>>,
    ) {
        self.set_witness_unsafe(inputs, hash.is_some(), hash.unwrap_or_default());
    }

    fn set_witness_unsafe<F: RichField>(
        &self,
        inputs: &mut PartialWitness<F>,
        hash_present: bool,
        hash: HashOut<F>,
    ) {
        inputs.set_bool_target(self.targets.inputs.hash_present, hash_present);
        inputs.set_hash_target(self.targets.inputs.hash, hash);
    }
}

pub struct BranchTargets<const D: usize> {
    /// The public inputs
    pub inputs: SubCircuitInputs,

    /// The left direction
    pub left: SubCircuitInputs,

    /// The right direction
    pub right: SubCircuitInputs,

    /// Whether or not the right direction is present
    pub partial: BoolTarget,

    /// The proof of event accumulation
    pub proof: ProofWithPublicInputsTarget<D>,
}

impl SubCircuitInputs {
    fn direction_from_node<const D: usize>(
        proof: &ProofWithPublicInputsTarget<D>,
        indices: &PublicIndices,
    ) -> SubCircuitInputs {
        let hash_present = indices.hash_present.get_target(&proof.public_inputs);
        let hash = indices.hash.get_target(&proof.public_inputs);

        SubCircuitInputs { hash_present, hash }
    }

    #[must_use]
    pub fn build_branch<F, C, const D: usize>(
        self,
        builder: &mut CircuitBuilder<F, D>,
        mc: &super::BranchCircuit<F, C, D>,
        indices: &PublicIndices,
        left_proof: &ProofWithPublicInputsTarget<D>,
        right_proof: &ProofWithPublicInputsTarget<D>,
    ) -> BranchTargets<D>
    where
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F>,
        C::Hasher: AlgebraicHasher<F>, {
        let _false = builder._false();
        let left = Self::direction_from_node(left_proof, indices);
        let right = Self::direction_from_node(right_proof, indices);
        let partial = builder.add_virtual_bool_target_safe();

        let circuit = &mc.circuit;
        let proof = builder.add_virtual_proof_with_pis(&circuit.common);
        let verifier = builder.constant_verifier_data(&circuit.verifier_only);
        builder.verify_proof::<C>(&proof, &verifier, &circuit.common);

        let a_present = mc.merge.indices.a_present.get_target(&proof.public_inputs);
        let b_present = mc.merge.indices.b_present.get_target(&proof.public_inputs);
        let merged_present = mc
            .merge
            .indices
            .merged_present
            .get_target(&proof.public_inputs);
        let a_hash = mc.merge.indices.a_hash.get_target(&proof.public_inputs);
        let b_hash = mc.merge.indices.b_hash.get_target(&proof.public_inputs);
        let merged_hash = mc
            .merge
            .indices
            .merged_hash
            .get_target(&proof.public_inputs);

        let b_present_calc = false_if(builder, partial, right.hash_present);
        let b_hash_calc = zero_hash_if(builder, partial, right.hash);

        builder.connect(a_present.target, left.hash_present.target);
        builder.connect(b_present.target, b_present_calc.target);
        builder.connect(merged_present.target, self.hash_present.target);
        builder.connect_hashes(a_hash, left.hash);
        builder.connect_hashes(b_hash, b_hash_calc);
        builder.connect_hashes(merged_hash, self.hash);

        BranchTargets {
            inputs: self,
            left,
            right,
            partial,
            proof,
        }
    }
}

pub struct BranchSubCircuit<const D: usize> {
    pub targets: BranchTargets<D>,
    pub indices: PublicIndices,
}

impl<const D: usize> BranchTargets<D> {
    #[must_use]
    pub fn build(self, child: &PublicIndices, public_inputs: &[Target]) -> BranchSubCircuit<D> {
        let indices = PublicIndices {
            hash_present: BoolTargetIndex::new(public_inputs, self.inputs.hash_present),
            hash: HashOutTargetIndex::new(public_inputs, self.inputs.hash),
        };
        debug_assert_eq!(indices, *child);

        BranchSubCircuit {
            indices,
            targets: self,
        }
    }
}

impl<const D: usize> BranchSubCircuit<D> {
    pub fn set_witness<F, C>(
        &self,
        inputs: &mut PartialWitness<F>,
        partial: bool,
        proof: &super::BranchProof<F, C, D>,
    ) where
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F>,
        <C as GenericConfig<D>>::Hasher: AlgebraicHasher<F>, {
        inputs.set_bool_target(self.targets.partial, partial);
        inputs.set_proof_with_pis_target(&self.targets.proof, &proof.proof);
    }

    pub fn set_witness_unsafe<F, C>(
        &self,
        inputs: &mut PartialWitness<F>,
        partial: bool,
        proof: &ProofWithPublicInputs<F, C, D>,
        hash_present: bool,
        hash: HashOut<F>,
    ) where
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F>,
        <C as GenericConfig<D>>::Hasher: AlgebraicHasher<F>, {
        inputs.set_bool_target(self.targets.partial, partial);
        inputs.set_proof_with_pis_target(&self.targets.proof, proof);
        inputs.set_bool_target(self.targets.inputs.hash_present, hash_present);
        inputs.set_hash_target(self.targets.inputs.hash, hash);
    }
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use plonky2::plonk::circuit_data::{CircuitConfig, CircuitData};

    use super::*;
    use crate::circuits::merge::test as merge;
    use crate::subcircuits::bounded;
    use crate::test_utils::{hash_branch, C, CONFIG, D, F, NON_ZERO_HASHES, ZERO_HASH};

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

        pub fn prove(&self, tree: Option<HashOut<F>>) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.bounded.set_witness(&mut inputs);
            self.merge.set_witness(&mut inputs, tree);
            self.circuit.prove(inputs)
        }

        #[allow(clippy::too_many_arguments)]
        pub fn prove_unsafe(
            &self,
            present: bool,
            hash: HashOut<F>,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.bounded.set_witness(&mut inputs);
            self.merge.set_witness_unsafe(&mut inputs, present, hash);
            self.circuit.prove(inputs)
        }
    }

    pub struct DummyBranchCircuit {
        pub bounded: bounded::BranchSubCircuit<D>,
        pub merge: BranchSubCircuit<D>,
        pub circuit: CircuitData<F, C, D>,
    }

    impl DummyBranchCircuit {
        #[must_use]
        pub fn new(
            circuit_config: &CircuitConfig,
            mc: &merge::BranchCircuit<F, C, D>,
            indices: &PublicIndices,
            child: &CircuitData<F, C, D>,
        ) -> Self {
            let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());

            let bounded_inputs = bounded::SubCircuitInputs::default(&mut builder);
            let merge_inputs = SubCircuitInputs::default(&mut builder);

            let bounded_targets = bounded_inputs.build_branch(&mut builder, child);
            let merge_targets = merge_inputs.build_branch(
                &mut builder,
                mc,
                indices,
                &bounded_targets.left_proof,
                &bounded_targets.right_proof,
            );

            let circuit = builder.build();

            let public_inputs = &circuit.prover_only.public_inputs;
            let bounded = bounded_targets.build(public_inputs);
            let merge = merge_targets.build(indices, public_inputs);
            Self {
                bounded,
                merge,
                circuit,
            }
        }

        #[must_use]
        pub fn from_leaf(
            circuit_config: &CircuitConfig,
            mc: &merge::BranchCircuit<F, C, D>,
            leaf: &DummyLeafCircuit,
        ) -> Self {
            Self::new(circuit_config, mc, &leaf.merge.indices, &leaf.circuit)
        }

        #[must_use]
        pub fn from_branch(
            circuit_config: &CircuitConfig,
            mc: &merge::BranchCircuit<F, C, D>,
            branch: &Self,
        ) -> Self {
            Self::new(circuit_config, mc, &branch.merge.indices, &branch.circuit)
        }

        pub fn prove(
            &self,
            merged_proof: &merge::BranchProof<F, C, D>,
            left_proof: &ProofWithPublicInputs<F, C, D>,
            right_proof: Option<&ProofWithPublicInputs<F, C, D>>,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.bounded
                .set_witness(&mut inputs, left_proof, right_proof.unwrap_or(left_proof));
            self.merge
                .set_witness(&mut inputs, right_proof.is_none(), merged_proof);
            self.circuit.prove(inputs)
        }

        #[allow(clippy::too_many_arguments)]
        pub fn prove_unsafe(
            &self,
            partial: bool,
            merged_proof: &ProofWithPublicInputs<F, C, D>,
            present: bool,
            hash: HashOut<F>,
            left_proof: &ProofWithPublicInputs<F, C, D>,
            right_proof: &ProofWithPublicInputs<F, C, D>,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.bounded
                .set_witness(&mut inputs, left_proof, right_proof);
            self.merge
                .set_witness_unsafe(&mut inputs, partial, merged_proof, present, hash);
            self.circuit.prove(inputs)
        }
    }

    #[tested_fixture::tested_fixture(pub LEAF)]
    fn build_leaf() -> DummyLeafCircuit { DummyLeafCircuit::new(&CONFIG) }

    #[tested_fixture::tested_fixture(pub BRANCH_1)]
    fn build_branch_1() -> DummyBranchCircuit {
        DummyBranchCircuit::from_leaf(&CONFIG, &merge::BRANCH, &LEAF)
    }

    #[tested_fixture::tested_fixture(pub BRANCH_2)]
    fn build_branch_2() -> DummyBranchCircuit {
        DummyBranchCircuit::from_branch(&CONFIG, &merge::BRANCH, &BRANCH_1)
    }

    fn assert_leaf(proof: &ProofWithPublicInputs<F, C, D>, hash: Option<HashOut<F>>) {
        let indices = &LEAF.merge.indices;

        let p_hash = indices.hash_present.get_field(&proof.public_inputs);
        assert_eq!(p_hash, hash.is_some());

        let p_hash = indices.hash.get_field(&proof.public_inputs);
        assert_eq!(p_hash, hash.unwrap_or_default());
    }

    fn assert_branch(proof: &ProofWithPublicInputs<F, C, D>, hash: Option<HashOut<F>>) {
        let indices = &LEAF.merge.indices;

        let p_hash = indices.hash_present.get_field(&proof.public_inputs);
        assert_eq!(p_hash, hash.is_some());

        let p_hash = indices.hash.get_field(&proof.public_inputs);
        assert_eq!(p_hash, hash.unwrap_or_default());
    }

    #[tested_fixture::tested_fixture(EMPTY_LEAF_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_empty_leaf() -> Result<ProofWithPublicInputs<F, C, D>> {
        let proof = LEAF.prove(None)?;
        assert_leaf(&proof, None);
        LEAF.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(BAD_EMPTY_LEAF_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_bad_empty_leaf() -> Result<ProofWithPublicInputs<F, C, D>> {
        // We allow non-correspondance here
        let proof = LEAF.prove_unsafe(false, NON_ZERO_HASHES[2])?;
        LEAF.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(ZERO_LEAF_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_zero_leaf() -> Result<ProofWithPublicInputs<F, C, D>> {
        let proof = LEAF.prove(Some(ZERO_HASH))?;
        assert_leaf(&proof, Some(ZERO_HASH));
        LEAF.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(NON_ZERO_LEAF_PROOF_1: ProofWithPublicInputs<F, C, D>)]
    fn verify_non_zero_leaf_1() -> Result<ProofWithPublicInputs<F, C, D>> {
        let proof = LEAF.prove(Some(NON_ZERO_HASHES[0]))?;
        assert_leaf(&proof, Some(NON_ZERO_HASHES[0]));
        LEAF.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(NON_ZERO_LEAF_PROOF_2: ProofWithPublicInputs<F, C, D>)]
    fn verify_non_zero_leaf_2() -> Result<ProofWithPublicInputs<F, C, D>> {
        let proof = LEAF.prove(Some(NON_ZERO_HASHES[1]))?;
        assert_leaf(&proof, Some(NON_ZERO_HASHES[1]));
        LEAF.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(EMPTY_BRANCH_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_empty_branch() -> Result<ProofWithPublicInputs<F, C, D>> {
        let proof = BRANCH_1.prove(
            *merge::EMPTY_BRANCH_PROOF,
            *EMPTY_LEAF_PROOF,
            Some(*EMPTY_LEAF_PROOF),
        )?;
        assert_branch(&proof, None);
        BRANCH_1.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(EMPTY_PARTIAL_BRANCH_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_empty_partial_branch() -> Result<ProofWithPublicInputs<F, C, D>> {
        let proof = BRANCH_1.prove(*merge::EMPTY_BRANCH_PROOF, *EMPTY_LEAF_PROOF, None)?;
        assert_branch(&proof, None);
        BRANCH_1.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(EMPTY_PARTIAL_BAD_BRANCH_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_empty_partial_bad_branch() -> Result<ProofWithPublicInputs<F, C, D>> {
        let proof = BRANCH_1.prove_unsafe(
            true,
            &merge::EMPTY_BRANCH_PROOF.proof,
            false,
            ZERO_HASH,
            *EMPTY_LEAF_PROOF,
            *BAD_EMPTY_LEAF_PROOF,
        )?;
        assert_branch(&proof, None);
        BRANCH_1.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(LEFT_BRANCH_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_left_branch() -> Result<ProofWithPublicInputs<F, C, D>> {
        let proof = BRANCH_1.prove(
            *merge::LEFT_BRANCH_PROOF,
            *NON_ZERO_LEAF_PROOF_1,
            Some(*EMPTY_LEAF_PROOF),
        )?;
        assert_branch(&proof, Some(NON_ZERO_HASHES[0]));
        BRANCH_1.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(LEFT_PARTIAL_BRANCH_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_left_partial_branch() -> Result<ProofWithPublicInputs<F, C, D>> {
        let proof = BRANCH_1.prove(*merge::LEFT_BRANCH_PROOF, *NON_ZERO_LEAF_PROOF_1, None)?;
        assert_branch(&proof, Some(NON_ZERO_HASHES[0]));
        BRANCH_1.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(RIGHT_BRANCH_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_right_branch() -> Result<ProofWithPublicInputs<F, C, D>> {
        let proof = BRANCH_1.prove(
            *merge::RIGHT_BRANCH_PROOF,
            *EMPTY_LEAF_PROOF,
            Some(*NON_ZERO_LEAF_PROOF_2),
        )?;
        assert_branch(&proof, Some(NON_ZERO_HASHES[1]));
        BRANCH_1.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[test]
    fn verify_both_branch() -> Result<()> {
        let proof = BRANCH_1.prove(
            &merge::BOTH_BRANCH_PROOF,
            *NON_ZERO_LEAF_PROOF_1,
            Some(*NON_ZERO_LEAF_PROOF_2),
        )?;
        assert_branch(
            &proof,
            Some(hash_branch(&NON_ZERO_HASHES[0], &NON_ZERO_HASHES[1])),
        );
        BRANCH_1.circuit.verify(proof)?;
        Ok(())
    }

    #[test]
    fn verify_both_double_branch() -> Result<()> {
        let proof = BRANCH_2.prove(
            &merge::BOTH_BRANCH_PROOF,
            *LEFT_BRANCH_PROOF,
            Some(*RIGHT_BRANCH_PROOF),
        )?;
        assert_branch(
            &proof,
            Some(hash_branch(&NON_ZERO_HASHES[0], &NON_ZERO_HASHES[1])),
        );
        BRANCH_2.circuit.verify(proof)?;
        Ok(())
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_branch_empty_1() {
        let proof = BRANCH_1
            .prove(
                *merge::EMPTY_BRANCH_PROOF,
                *EMPTY_LEAF_PROOF,
                Some(*ZERO_LEAF_PROOF),
            )
            .unwrap();
        BRANCH_1.circuit.verify(proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_branch_empty_2() {
        let proof = BRANCH_1
            .prove(
                *merge::EMPTY_BRANCH_PROOF,
                *EMPTY_LEAF_PROOF,
                Some(*BAD_EMPTY_LEAF_PROOF),
            )
            .unwrap();
        BRANCH_1.circuit.verify(proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_branch_empty_3() {
        let proof = BRANCH_1
            .prove(
                *merge::EMPTY_BRANCH_PROOF,
                *BAD_EMPTY_LEAF_PROOF,
                Some(*EMPTY_LEAF_PROOF),
            )
            .unwrap();
        BRANCH_1.circuit.verify(proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_branch_empty_4() {
        let proof = BRANCH_1
            .prove(*merge::EMPTY_BRANCH_PROOF, *BAD_EMPTY_LEAF_PROOF, None)
            .unwrap();
        BRANCH_1.circuit.verify(proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_branch_empty_5() {
        let proof = BRANCH_1
            .prove(
                *merge::LEFT_ZERO_BRANCH_PROOF,
                *EMPTY_LEAF_PROOF,
                Some(*EMPTY_LEAF_PROOF),
            )
            .unwrap();
        BRANCH_1.circuit.verify(proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_branch_empty_6() {
        let proof = BRANCH_1
            .prove(
                *merge::RIGHT_ZERO_BRANCH_PROOF,
                *EMPTY_LEAF_PROOF,
                Some(*EMPTY_LEAF_PROOF),
            )
            .unwrap();
        BRANCH_1.circuit.verify(proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_right_partial_branch_1() {
        let proof = BRANCH_1
            .prove(*merge::RIGHT_BRANCH_PROOF, *EMPTY_LEAF_PROOF, None)
            .unwrap();
        BRANCH_1.circuit.verify(proof.clone()).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_right_partial_branch_2() {
        let proof = BRANCH_1
            .prove(*merge::RIGHT_BRANCH_PROOF, *NON_ZERO_LEAF_PROOF_2, None)
            .unwrap();
        BRANCH_1.circuit.verify(proof.clone()).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_branch_pair() {
        let proof = BRANCH_1
            .prove_unsafe(
                false,
                &merge::BOTH_BRANCH_PROOF.proof,
                true,
                hash_branch(&NON_ZERO_HASHES[1], &NON_ZERO_HASHES[0]),
                *NON_ZERO_LEAF_PROOF_1,
                *NON_ZERO_LEAF_PROOF_2,
            )
            .unwrap();
        BRANCH_1.circuit.verify(proof).unwrap();
    }
}

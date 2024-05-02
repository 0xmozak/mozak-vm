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

use crate::indices::{BoolTargetIndex, HashTargetIndex};

/// The indices of the public inputs of this subcircuit in any
/// `ProofWithPublicInputs`
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct PublicIndices {
    /// The index for the presence of the hash
    pub hash_present: BoolTargetIndex,

    /// The indices of each of the elements of the hash
    pub hash: HashTargetIndex,
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
            hash: HashTargetIndex::new(public_inputs, self.inputs.hash),
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

    /// The proof of event accumulation
    pub proof: ProofWithPublicInputsTarget<D>,
}

impl SubCircuitInputs {
    fn direction_from_node<const D: usize>(
        proof: &ProofWithPublicInputsTarget<D>,
        indices: &PublicIndices,
    ) -> SubCircuitInputs {
        let hash_present = indices.hash_present.get(&proof.public_inputs);
        let hash = indices.hash.get(&proof.public_inputs);

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
        let left = Self::direction_from_node(left_proof, indices);
        let right = Self::direction_from_node(right_proof, indices);

        let circuit = &mc.circuit;
        let proof = builder.add_virtual_proof_with_pis(&circuit.common);
        let verifier = builder.constant_verifier_data(&circuit.verifier_only);
        builder.verify_proof::<C>(&proof, &verifier, &circuit.common);

        let a_present = mc.merge.indices.a_present.get_any(&proof.public_inputs);
        let b_present = mc.merge.indices.b_present.get_any(&proof.public_inputs);
        let merged_present = mc
            .merge
            .indices
            .merged_present
            .get_any(&proof.public_inputs);
        let a_hash = mc.merge.indices.a_hash.get(&proof.public_inputs);
        let b_hash = mc.merge.indices.b_hash.get(&proof.public_inputs);
        let merged_hash = mc.merge.indices.merged_hash.get(&proof.public_inputs);
        builder.connect(a_present, left.hash_present.target);
        builder.connect(b_present, right.hash_present.target);
        builder.connect(merged_present, self.hash_present.target);
        builder.connect_hashes(a_hash, left.hash);
        builder.connect_hashes(b_hash, right.hash);
        builder.connect_hashes(merged_hash, self.hash);

        BranchTargets {
            inputs: self,
            left,
            right,
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
            hash: HashTargetIndex::new(public_inputs, self.inputs.hash),
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
        proof: &ProofWithPublicInputs<F, C, D>,
    ) where
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F>,
        <C as GenericConfig<D>>::Hasher: AlgebraicHasher<F>, {
        inputs.set_proof_with_pis_target(&self.targets.proof, proof);
    }

    pub fn set_witness_unsafe<F, C>(
        &self,
        inputs: &mut PartialWitness<F>,
        proof: &ProofWithPublicInputs<F, C, D>,
        hash_present: bool,
        hash: HashOut<F>,
    ) where
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F>,
        <C as GenericConfig<D>>::Hasher: AlgebraicHasher<F>, {
        inputs.set_proof_with_pis_target(&self.targets.proof, proof);
        inputs.set_bool_target(self.targets.inputs.hash_present, hash_present);
        inputs.set_hash_target(self.targets.inputs.hash, hash);
    }
}

#[cfg(test)]
mod test {
    use std::panic::catch_unwind;

    use anyhow::Result;
    use lazy_static::lazy_static;
    use plonky2::field::types::Field;
    use plonky2::hash::hash_types::NUM_HASH_OUT_ELTS;
    use plonky2::plonk::circuit_data::{CircuitConfig, CircuitData};

    use super::*;
    use crate::circuits::merge::{
        BranchCircuit as MergeBranchCircuit, LeafCircuit as MergeLeafCircuit,
    };
    use crate::subcircuits::bounded;
    use crate::test_utils::{hash_branch, hash_str, C, CONFIG, D, F};

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
            mc: &MergeBranchCircuit<F, C, D>,
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
            mc: &MergeBranchCircuit<F, C, D>,
            leaf: &DummyLeafCircuit,
        ) -> Self {
            Self::new(circuit_config, mc, &leaf.merge.indices, &leaf.circuit)
        }

        #[must_use]
        pub fn from_branch(
            circuit_config: &CircuitConfig,
            mc: &MergeBranchCircuit<F, C, D>,
            branch: &Self,
        ) -> Self {
            Self::new(circuit_config, mc, &branch.merge.indices, &branch.circuit)
        }

        pub fn prove(
            &self,
            merged_proof: &ProofWithPublicInputs<F, C, D>,
            left_proof: &ProofWithPublicInputs<F, C, D>,
            right_proof: &ProofWithPublicInputs<F, C, D>,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.bounded
                .set_witness(&mut inputs, left_proof, right_proof);
            self.merge.set_witness(&mut inputs, merged_proof);
            self.circuit.prove(inputs)
        }

        #[allow(clippy::too_many_arguments)]
        pub fn prove_unsafe(
            &self,
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
                .set_witness_unsafe(&mut inputs, merged_proof, present, hash);
            self.circuit.prove(inputs)
        }
    }

    lazy_static! {
        static ref MC_LEAF: MergeLeafCircuit<F, C, D> = MergeLeafCircuit::new(&CONFIG);
        static ref MC_BRANCH: MergeBranchCircuit<F, C, D> =
            MergeBranchCircuit::new(&CONFIG, &MC_LEAF);
        static ref LEAF: DummyLeafCircuit = DummyLeafCircuit::new(&CONFIG);
        static ref BRANCH_1: DummyBranchCircuit =
            DummyBranchCircuit::from_leaf(&CONFIG, &MC_BRANCH, &LEAF);
        static ref BRANCH_2: DummyBranchCircuit =
            DummyBranchCircuit::from_branch(&CONFIG, &MC_BRANCH, &BRANCH_1);
    }

    #[test]
    fn verify_leaf() -> Result<()> {
        let zero_hash = HashOut::from([F::ZERO; NUM_HASH_OUT_ELTS]);
        let a_val = hash_str("Value Alpha");
        let b_val = hash_str("Value Beta");

        let proof = LEAF.prove(None)?;
        LEAF.circuit.verify(proof)?;

        let proof = LEAF.prove(Some(zero_hash))?;
        LEAF.circuit.verify(proof)?;

        let proof = LEAF.prove(Some(a_val))?;
        LEAF.circuit.verify(proof)?;

        let proof = LEAF.prove(Some(b_val))?;
        LEAF.circuit.verify(proof)?;

        Ok(())
    }

    #[test]
    fn verify_branch_empty() -> Result<()> {
        let zero_hash = HashOut::from([F::ZERO; NUM_HASH_OUT_ELTS]);

        let empty_merge_leaf = MC_LEAF.prove(&MC_BRANCH, None, None, Some(zero_hash))?;
        MC_LEAF.circuit.verify(empty_merge_leaf.clone())?;

        let merge_branch = MC_BRANCH.prove(true, &empty_merge_leaf, true, &empty_merge_leaf)?;
        MC_BRANCH.circuit.verify(merge_branch.clone())?;

        let empty_proof = LEAF.prove(None)?;
        LEAF.circuit.verify(empty_proof.clone())?;

        let branch_proof = BRANCH_1.prove(&merge_branch, &empty_proof, &empty_proof)?;
        BRANCH_1.circuit.verify(branch_proof)?;

        Ok(())
    }

    #[test]
    fn verify_branch_single() -> Result<()> {
        let zero_hash = HashOut::from([F::ZERO; NUM_HASH_OUT_ELTS]);
        let a_val = hash_str("Value Alpha");
        let b_val = hash_str("Value Beta");

        let empty_merge_leaf = MC_LEAF.prove(&MC_BRANCH, None, None, Some(zero_hash))?;
        MC_LEAF.circuit.verify(empty_merge_leaf.clone())?;

        let a_merge_leaf = MC_LEAF.prove(&MC_BRANCH, Some(a_val), None, Some(a_val))?;
        MC_LEAF.circuit.verify(a_merge_leaf.clone())?;

        let b_merge_leaf = MC_LEAF.prove(&MC_BRANCH, None, Some(b_val), Some(b_val))?;
        MC_LEAF.circuit.verify(b_merge_leaf.clone())?;

        let merge_branch_a = MC_BRANCH.prove(true, &a_merge_leaf, true, &empty_merge_leaf)?;
        MC_BRANCH.circuit.verify(merge_branch_a.clone())?;

        let merge_branch_b = MC_BRANCH.prove(true, &empty_merge_leaf, true, &b_merge_leaf)?;
        MC_BRANCH.circuit.verify(merge_branch_b.clone())?;

        let merge_branch_ab = MC_BRANCH.prove(false, &merge_branch_a, false, &merge_branch_b)?;
        MC_BRANCH.circuit.verify(merge_branch_b.clone())?;

        let empty_proof = LEAF.prove(None)?;
        LEAF.circuit.verify(empty_proof.clone())?;

        let a_proof = LEAF.prove(Some(a_val))?;
        LEAF.circuit.verify(a_proof.clone())?;

        let b_proof = LEAF.prove(Some(b_val))?;
        LEAF.circuit.verify(b_proof.clone())?;

        let a_branch_proof = BRANCH_1.prove(&merge_branch_a, &a_proof, &empty_proof)?;
        BRANCH_1.circuit.verify(a_branch_proof.clone())?;

        let b_branch_proof = BRANCH_1.prove(&merge_branch_b, &empty_proof, &b_proof)?;
        BRANCH_1.circuit.verify(b_branch_proof.clone())?;

        let ab_branch_proof = BRANCH_2.prove(&merge_branch_ab, &a_branch_proof, &b_branch_proof)?;
        BRANCH_2.circuit.verify(ab_branch_proof.clone())?;

        Ok(())
    }

    #[test]
    fn verify_branch_pair() -> Result<()> {
        let a_val = hash_str("Value Alpha");
        let b_val = hash_str("Value Beta");

        let a_merge_leaf = MC_LEAF.prove(&MC_BRANCH, Some(a_val), None, Some(a_val))?;
        MC_LEAF.circuit.verify(a_merge_leaf.clone())?;

        let b_merge_leaf = MC_LEAF.prove(&MC_BRANCH, None, Some(b_val), Some(b_val))?;
        MC_LEAF.circuit.verify(b_merge_leaf.clone())?;

        let merge_branch = MC_BRANCH.prove(true, &a_merge_leaf, true, &b_merge_leaf)?;
        MC_BRANCH.circuit.verify(merge_branch.clone())?;

        let a_proof = LEAF.prove(Some(a_val))?;
        LEAF.circuit.verify(a_proof.clone())?;

        let b_proof = LEAF.prove(Some(b_val))?;
        LEAF.circuit.verify(b_proof.clone())?;

        let branch_proof = BRANCH_1.prove(&merge_branch, &a_proof, &b_proof)?;
        BRANCH_1.circuit.verify(branch_proof)?;

        Ok(())
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_branch_empty_1() {
        let (merge_branch, good_empty_proof, bad_empty_proof) = catch_unwind(|| {
            let zero_hash = HashOut::from([F::ZERO; NUM_HASH_OUT_ELTS]);

            let empty_merge_leaf = MC_LEAF
                .prove(&MC_BRANCH, None, None, Some(zero_hash))
                .unwrap();
            MC_LEAF.circuit.verify(empty_merge_leaf.clone()).unwrap();

            let merge_branch = MC_BRANCH
                .prove(true, &empty_merge_leaf, true, &empty_merge_leaf)
                .unwrap();
            MC_BRANCH.circuit.verify(merge_branch.clone()).unwrap();

            let good_empty_proof = LEAF.prove(None).unwrap();
            LEAF.circuit.verify(good_empty_proof.clone()).unwrap();

            let bad_empty_proof = LEAF.prove(Some(zero_hash)).unwrap();
            LEAF.circuit.verify(bad_empty_proof.clone()).unwrap();

            (merge_branch, good_empty_proof, bad_empty_proof)
        })
        .expect("shouldn't fail");

        let branch_proof = BRANCH_1
            .prove(&merge_branch, &good_empty_proof, &bad_empty_proof)
            .unwrap();
        BRANCH_1.circuit.verify(branch_proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_branch_empty_2() {
        let (merge_branch, good_empty_proof, bad_empty_proof) = catch_unwind(|| {
            let zero_hash = HashOut::from([F::ZERO; NUM_HASH_OUT_ELTS]);
            let non_zero_hash_1 = hash_str("Non-Zero Hash 1");

            let empty_merge_leaf = MC_LEAF
                .prove(&MC_BRANCH, None, None, Some(zero_hash))
                .unwrap();
            MC_LEAF.circuit.verify(empty_merge_leaf.clone()).unwrap();

            let merge_branch = MC_BRANCH
                .prove(true, &empty_merge_leaf, true, &empty_merge_leaf)
                .unwrap();
            MC_BRANCH.circuit.verify(merge_branch.clone()).unwrap();

            let good_empty_proof = LEAF.prove(None).unwrap();
            LEAF.circuit.verify(good_empty_proof.clone()).unwrap();

            let bad_empty_proof = LEAF.prove_unsafe(false, non_zero_hash_1).unwrap();
            LEAF.circuit.verify(bad_empty_proof.clone()).unwrap();

            (merge_branch, good_empty_proof, bad_empty_proof)
        })
        .expect("shouldn't fail");

        let branch_proof = BRANCH_1
            .prove(&merge_branch, &good_empty_proof, &bad_empty_proof)
            .unwrap();
        BRANCH_1.circuit.verify(branch_proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_branch_empty_3() {
        let (merge_branch, empty_proof) = catch_unwind(|| {
            let zero_hash = HashOut::from([F::ZERO; NUM_HASH_OUT_ELTS]);

            let empty_merge_leaf = MC_LEAF
                .prove(&MC_BRANCH, None, None, Some(zero_hash))
                .unwrap();
            MC_LEAF.circuit.verify(empty_merge_leaf.clone()).unwrap();

            let bad_empty_merge_leaf = MC_LEAF
                .prove(&MC_BRANCH, Some(zero_hash), None, Some(zero_hash))
                .unwrap();
            MC_LEAF
                .circuit
                .verify(bad_empty_merge_leaf.clone())
                .unwrap();

            let merge_branch = MC_BRANCH
                .prove(true, &empty_merge_leaf, true, &bad_empty_merge_leaf)
                .unwrap();
            MC_BRANCH.circuit.verify(merge_branch.clone()).unwrap();

            let empty_proof = LEAF.prove(None).unwrap();
            LEAF.circuit.verify(empty_proof.clone()).unwrap();

            (merge_branch, empty_proof)
        })
        .expect("shouldn't fail");

        let branch_proof = BRANCH_1
            .prove(&merge_branch, &empty_proof, &empty_proof)
            .unwrap();
        BRANCH_1.circuit.verify(branch_proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_branch_pair() {
        let (merge_branch, ba_hash, a_proof, b_proof) = catch_unwind(|| {
            let a_val = hash_str("Value Alpha");
            let b_val = hash_str("Value Beta");
            let ba_hash = hash_branch(&b_val, &a_val);

            let a_merge_leaf = MC_LEAF
                .prove(&MC_BRANCH, Some(a_val), None, Some(a_val))
                .unwrap();
            MC_LEAF.circuit.verify(a_merge_leaf.clone()).unwrap();

            let b_merge_leaf = MC_LEAF
                .prove(&MC_BRANCH, None, Some(b_val), Some(b_val))
                .unwrap();
            MC_LEAF.circuit.verify(b_merge_leaf.clone()).unwrap();

            let merge_branch = MC_BRANCH
                .prove(true, &a_merge_leaf, true, &b_merge_leaf)
                .unwrap();
            MC_BRANCH.circuit.verify(merge_branch.clone()).unwrap();

            let a_proof = LEAF.prove(Some(a_val)).unwrap();
            LEAF.circuit.verify(a_proof.clone()).unwrap();

            let b_proof = LEAF.prove(Some(b_val)).unwrap();
            LEAF.circuit.verify(b_proof.clone()).unwrap();
            (merge_branch, ba_hash, a_proof, b_proof)
        })
        .expect("shouldn't fail");

        let branch_proof = BRANCH_1
            .prove_unsafe(&merge_branch, true, ba_hash, &a_proof, &b_proof)
            .unwrap();
        BRANCH_1.circuit.verify(branch_proof).unwrap();
    }
}

//! Subcircuits for recursively proving partial contents of a merkle tree.
//!
//! These can be used to prove a subset of nodes belong to a tree.

use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::{HashOut, HashOutTarget, RichField};
use plonky2::iop::target::{BoolTarget, Target};
use plonky2::iop::witness::{PartialWitness, WitnessWrite};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::proof::ProofWithPublicInputsTarget;

use crate::hash_or_forward;
use crate::indices::{BoolTargetIndex, HashTargetIndex};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct PublicIndices {
    pub summary_hash_present: BoolTargetIndex,
    pub summary_hash: HashTargetIndex,
}

pub struct SubCircuitInputs {
    pub summary_hash_present: BoolTarget,

    /// The hash of the previous state or ZERO if absent
    ///
    /// For branches this is defined as follows:
    /// `hash([left.summary_hash, right.summary_hash])` if both present
    /// `x.summary_hash` if only one is present
    /// ZERO if both are absent
    pub summary_hash: HashOutTarget,
}

pub struct LeafTargets {
    /// The public inputs
    pub inputs: SubCircuitInputs,
}

impl SubCircuitInputs {
    pub fn default<F, const D: usize>(builder: &mut CircuitBuilder<F, D>) -> Self
    where
        F: RichField + Extendable<D>, {
        let summary_hash_present = builder.add_virtual_bool_target_safe();
        let summary_hash = builder.add_virtual_hash();
        builder.register_public_input(summary_hash_present.target);
        builder.register_public_inputs(&summary_hash.elements);
        Self {
            summary_hash_present,
            summary_hash,
        }
    }

    #[must_use]
    pub fn build_leaf<F, const D: usize>(self, builder: &mut CircuitBuilder<F, D>) -> LeafTargets
    where
        F: RichField + Extendable<D>, {
        // prove hashes align with presence
        for e in self.summary_hash.elements {
            let e = builder.is_nonzero(e);
            builder.connect(e.target, self.summary_hash_present.target);
        }

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
            summary_hash_present: BoolTargetIndex::new(
                public_inputs,
                self.inputs.summary_hash_present,
            ),
            summary_hash: HashTargetIndex::new(public_inputs, self.inputs.summary_hash),
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
        summary_hash: HashOut<F>,
    ) {
        self.set_witness_unsafe(inputs, summary_hash != HashOut::ZERO, summary_hash);
    }

    fn set_witness_unsafe<F: RichField>(
        &self,
        inputs: &mut PartialWitness<F>,
        summary_hash_present: bool,
        summary_hash: HashOut<F>,
    ) {
        inputs.set_bool_target(
            self.targets.inputs.summary_hash_present,
            summary_hash_present,
        );
        inputs.set_hash_target(self.targets.inputs.summary_hash, summary_hash);
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
    fn direction_from_node<const D: usize>(
        proof: &ProofWithPublicInputsTarget<D>,
        indices: &PublicIndices,
    ) -> SubCircuitInputs {
        let summary_hash_present = indices.summary_hash_present.get(&proof.public_inputs);
        let summary_hash = indices.summary_hash.get(&proof.public_inputs);

        SubCircuitInputs {
            summary_hash_present,
            summary_hash,
        }
    }

    #[must_use]
    pub fn build_branch<F: RichField + Extendable<D>, const D: usize>(
        self,
        builder: &mut CircuitBuilder<F, D>,
        indices: &PublicIndices,
        left_proof: &ProofWithPublicInputsTarget<D>,
        right_proof: &ProofWithPublicInputsTarget<D>,
    ) -> BranchTargets {
        let left = Self::direction_from_node(left_proof, indices);
        let right = Self::direction_from_node(right_proof, indices);

        let l_present = left.summary_hash_present;
        let l_hash = left.summary_hash.elements;
        let r_present = right.summary_hash_present;
        let r_hash = right.summary_hash.elements;

        // Construct the forwarding "hash".
        let summary_hash_calc = hash_or_forward(builder, l_present, l_hash, r_present, r_hash);
        let summary_hash_present_calc = builder.or(l_present, r_present);

        builder.connect(
            summary_hash_present_calc.target,
            self.summary_hash_present.target,
        );
        builder.connect_hashes(summary_hash_calc, self.summary_hash);

        BranchTargets {
            inputs: self,
            left,
            right,
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
        let indices = PublicIndices {
            summary_hash_present: BoolTargetIndex::new(
                public_inputs,
                self.inputs.summary_hash_present,
            ),
            summary_hash: HashTargetIndex::new(public_inputs, self.inputs.summary_hash),
        };
        debug_assert_eq!(indices, *child);

        BranchSubCircuit {
            indices,
            targets: self,
        }
    }
}

impl BranchSubCircuit {
    pub fn set_witness<F: RichField>(
        &self,
        inputs: &mut PartialWitness<F>,
        summary_hash: HashOut<F>,
    ) {
        self.set_witness_unsafe(inputs, summary_hash != HashOut::ZERO, summary_hash);
    }

    fn set_witness_unsafe<F: RichField>(
        &self,
        inputs: &mut PartialWitness<F>,
        summary_hash_present: bool,
        summary_hash: HashOut<F>,
    ) {
        inputs.set_bool_target(
            self.targets.inputs.summary_hash_present,
            summary_hash_present,
        );
        inputs.set_hash_target(self.targets.inputs.summary_hash, summary_hash);
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
        pub summarized: LeafSubCircuit,
        pub circuit: CircuitData<F, C, D>,
    }

    impl DummyLeafCircuit {
        #[must_use]
        pub fn new(circuit_config: &CircuitConfig) -> Self {
            let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());

            let bounded_inputs = bounded::SubCircuitInputs::default(&mut builder);
            let summarized_inputs = SubCircuitInputs::default(&mut builder);

            let bounded_targets = bounded_inputs.build_leaf(&mut builder);
            let summarized_targets = summarized_inputs.build_leaf(&mut builder);

            let circuit = builder.build();

            let public_inputs = &circuit.prover_only.public_inputs;
            let bounded = bounded_targets.build(public_inputs);
            let summarized = summarized_targets.build(public_inputs);

            Self {
                bounded,
                summarized,
                circuit,
            }
        }

        pub fn prove(&self, summary_hash: HashOut<F>) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.bounded.set_witness(&mut inputs);
            self.summarized.set_witness(&mut inputs, summary_hash);
            self.circuit.prove(inputs)
        }

        fn prove_unsafe(
            &self,
            summary_hash_present: bool,
            summary_hash: HashOut<F>,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.bounded.set_witness(&mut inputs);
            self.summarized
                .set_witness_unsafe(&mut inputs, summary_hash_present, summary_hash);
            self.circuit.prove(inputs)
        }
    }

    pub struct DummyBranchCircuit {
        pub bounded: bounded::BranchSubCircuit<D>,
        pub summarized: BranchSubCircuit,
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
            let summarized_inputs = SubCircuitInputs::default(&mut builder);

            let bounded_targets = bounded_inputs.build_branch(&mut builder, child);
            let summarized_targets = summarized_inputs.build_branch(
                &mut builder,
                indices,
                &bounded_targets.left_proof,
                &bounded_targets.right_proof,
            );

            let circuit = builder.build();

            let public_inputs = &circuit.prover_only.public_inputs;
            let bounded = bounded_targets.build(public_inputs);
            let summarized = summarized_targets.build(indices, public_inputs);

            Self {
                bounded,
                summarized,
                circuit,
            }
        }

        #[must_use]
        pub fn from_leaf(circuit_config: &CircuitConfig, leaf: &DummyLeafCircuit) -> Self {
            Self::new(circuit_config, &leaf.summarized.indices, &leaf.circuit)
        }

        #[must_use]
        pub fn from_branch(circuit_config: &CircuitConfig, branch: &Self) -> Self {
            Self::new(circuit_config, &branch.summarized.indices, &branch.circuit)
        }

        pub fn prove(
            &self,
            summary_hash: HashOut<F>,
            left_proof: &ProofWithPublicInputs<F, C, D>,
            right_proof: &ProofWithPublicInputs<F, C, D>,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.bounded
                .set_witness(&mut inputs, left_proof, right_proof);
            self.summarized.set_witness(&mut inputs, summary_hash);
            self.circuit.prove(inputs)
        }

        fn prove_unsafe(
            &self,
            summary_hash_present: bool,
            summary_hash: HashOut<F>,
            left_proof: &ProofWithPublicInputs<F, C, D>,
            right_proof: &ProofWithPublicInputs<F, C, D>,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.bounded
                .set_witness(&mut inputs, left_proof, right_proof);
            self.summarized
                .set_witness_unsafe(&mut inputs, summary_hash_present, summary_hash);
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
        let zero_hash = HashOut::from([F::ZERO; NUM_HASH_OUT_ELTS]);
        let non_zero_hash = hash_str("Non-Zero Hash");

        let proof = LEAF.prove(zero_hash)?;
        LEAF.circuit.verify(proof)?;

        let proof = LEAF.prove(non_zero_hash)?;
        LEAF.circuit.verify(proof)?;

        Ok(())
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_zero_leaf() {
        let zero_hash = HashOut::from([F::ZERO; NUM_HASH_OUT_ELTS]);

        let proof = LEAF.prove_unsafe(true, zero_hash).unwrap();
        LEAF.circuit.verify(proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_non_zero_leaf() {
        let non_zero_hash = hash_str("Non-Zero Hash");

        let proof = LEAF.prove_unsafe(false, non_zero_hash).unwrap();
        LEAF.circuit.verify(proof).unwrap();
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn verify_branch() -> Result<()> {
        let zero_hash = HashOut::from([F::ZERO; NUM_HASH_OUT_ELTS]);
        let non_zero_hash_1 = hash_str("Non-Zero Hash 1");
        let non_zero_hash_2 = hash_str("Non-Zero Hash 2");
        let both_hash = hash_branch(&non_zero_hash_1, &non_zero_hash_2);

        // Leaf proofs
        let zero_proof = LEAF.prove(zero_hash)?;
        LEAF.circuit.verify(zero_proof.clone())?;

        let non_zero_proof_1 = LEAF.prove(non_zero_hash_1)?;
        LEAF.circuit.verify(non_zero_proof_1.clone())?;

        let non_zero_proof_2 = LEAF.prove(non_zero_hash_2)?;
        LEAF.circuit.verify(non_zero_proof_2.clone())?;

        // Branch proofs
        let empty_branch_proof = BRANCH_1.prove(zero_hash, &zero_proof, &zero_proof)?;
        BRANCH_1.circuit.verify(empty_branch_proof.clone())?;

        let left1_branch_proof = BRANCH_1.prove(non_zero_hash_1, &non_zero_proof_1, &zero_proof)?;
        BRANCH_1.circuit.verify(left1_branch_proof.clone())?;

        let left2_branch_proof = BRANCH_1.prove(non_zero_hash_2, &non_zero_proof_2, &zero_proof)?;
        BRANCH_1.circuit.verify(left2_branch_proof.clone())?;

        let right1_branch_proof =
            BRANCH_1.prove(non_zero_hash_1, &zero_proof, &non_zero_proof_1)?;
        BRANCH_1.circuit.verify(right1_branch_proof.clone())?;

        let right2_branch_proof =
            BRANCH_1.prove(non_zero_hash_2, &zero_proof, &non_zero_proof_2)?;
        BRANCH_1.circuit.verify(right2_branch_proof.clone())?;

        let both_branch_proof = BRANCH_1.prove(both_hash, &non_zero_proof_1, &non_zero_proof_2)?;
        BRANCH_1.circuit.verify(both_branch_proof.clone())?;

        // Double branch proofs
        let empty_branch_2_proof =
            BRANCH_2.prove(zero_hash, &empty_branch_proof, &empty_branch_proof)?;
        BRANCH_2.circuit.verify(empty_branch_2_proof)?;

        let left_branch_2_proof =
            BRANCH_2.prove(non_zero_hash_1, &left1_branch_proof, &empty_branch_proof)?;
        BRANCH_2.circuit.verify(left_branch_2_proof)?;

        let left_branch_2_proof =
            BRANCH_2.prove(non_zero_hash_1, &empty_branch_proof, &left1_branch_proof)?;
        BRANCH_2.circuit.verify(left_branch_2_proof)?;

        let right_branch_2_proof =
            BRANCH_2.prove(non_zero_hash_2, &right2_branch_proof, &empty_branch_proof)?;
        BRANCH_2.circuit.verify(right_branch_2_proof)?;

        let right_branch_2_proof =
            BRANCH_2.prove(non_zero_hash_2, &empty_branch_proof, &right2_branch_proof)?;
        BRANCH_2.circuit.verify(right_branch_2_proof)?;

        let both_branch_2_proof =
            BRANCH_2.prove(both_hash, &left1_branch_proof, &left2_branch_proof)?;
        BRANCH_2.circuit.verify(both_branch_2_proof)?;

        let both_branch_2_proof =
            BRANCH_2.prove(both_hash, &left1_branch_proof, &right2_branch_proof)?;
        BRANCH_2.circuit.verify(both_branch_2_proof)?;

        let both_branch_2_proof =
            BRANCH_2.prove(both_hash, &right1_branch_proof, &left2_branch_proof)?;
        BRANCH_2.circuit.verify(both_branch_2_proof)?;

        let both_branch_2_proof =
            BRANCH_2.prove(both_hash, &right1_branch_proof, &right2_branch_proof)?;
        BRANCH_2.circuit.verify(both_branch_2_proof)?;

        let both_branch_2_proof =
            BRANCH_2.prove(both_hash, &both_branch_proof, &empty_branch_proof)?;
        BRANCH_2.circuit.verify(both_branch_2_proof)?;

        let both_branch_2_proof =
            BRANCH_2.prove(both_hash, &empty_branch_proof, &both_branch_proof)?;
        BRANCH_2.circuit.verify(both_branch_2_proof)?;

        Ok(())
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_proof_branch() {
        let zero_hash = HashOut::from([F::ZERO; NUM_HASH_OUT_ELTS]);

        let zero_proof = LEAF.prove(zero_hash).unwrap();
        LEAF.circuit.verify(zero_proof.clone()).unwrap();

        let bad_proof = LEAF.prove_unsafe(true, zero_hash).unwrap();

        let empty_branch_proof = BRANCH_1.prove(zero_hash, &zero_proof, &bad_proof).unwrap();
        BRANCH_1.circuit.verify(empty_branch_proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_zero_branch() {
        let zero_hash = HashOut::from([F::ZERO; NUM_HASH_OUT_ELTS]);

        let zero_proof = LEAF.prove(zero_hash).unwrap();
        LEAF.circuit.verify(zero_proof.clone()).unwrap();

        let branch_proof = BRANCH_1
            .prove_unsafe(true, zero_hash, &zero_proof, &zero_proof)
            .unwrap();
        BRANCH_1.circuit.verify(branch_proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_non_zero_branch() {
        let zero_hash = HashOut::from([F::ZERO; NUM_HASH_OUT_ELTS]);
        let non_zero_hash = hash_str("Non-Zero Hash");

        let zero_proof = LEAF.prove(zero_hash).unwrap();
        LEAF.circuit.verify(zero_proof.clone()).unwrap();

        let non_zero_proof = LEAF.prove(non_zero_hash).unwrap();
        LEAF.circuit.verify(non_zero_proof.clone()).unwrap();

        let branch_proof = BRANCH_1
            .prove_unsafe(false, non_zero_hash, &zero_proof, &non_zero_proof)
            .unwrap();
        BRANCH_1.circuit.verify(branch_proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_wrong_hash_branch() {
        let non_zero_hash_1 = hash_str("Non-Zero Hash 1");
        let non_zero_hash_2 = hash_str("Non-Zero Hash 2");

        let non_zero_proof_1 = LEAF.prove(non_zero_hash_1).unwrap();
        LEAF.circuit.verify(non_zero_proof_1.clone()).unwrap();

        let non_zero_proof_2 = LEAF.prove(non_zero_hash_2).unwrap();
        LEAF.circuit.verify(non_zero_proof_2.clone()).unwrap();

        let branch_proof = BRANCH_1
            .prove(non_zero_hash_1, &non_zero_proof_1, &non_zero_proof_2)
            .unwrap();
        BRANCH_1.circuit.verify(branch_proof).unwrap();
    }
}

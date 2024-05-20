//! Subcircuits for recursively proving partial contents of a merkle tree.
//!
//! These can be used to prove a subset of nodes belong to a tree.

use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::{HashOut, HashOutTarget, RichField};
use plonky2::iop::target::{BoolTarget, Target};
use plonky2::iop::witness::{PartialWitness, WitnessWrite};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::proof::ProofWithPublicInputsTarget;

use crate::indices::{BoolTargetIndex, HashOutTargetIndex};
use crate::{at_least_one_true, hash_is_zero, hash_or_forward};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct PublicIndices {
    pub summary_hash_present: BoolTargetIndex,
    pub summary_hash: HashOutTargetIndex,
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
        let hash_zero = hash_is_zero(builder, self.summary_hash);
        at_least_one_true(builder, [hash_zero, self.summary_hash_present]);
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
            summary_hash: HashOutTargetIndex::new(public_inputs, self.inputs.summary_hash),
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
        summary_hash: Option<HashOut<F>>,
    ) {
        self.set_witness_unsafe(
            inputs,
            summary_hash.is_some(),
            summary_hash.unwrap_or(HashOut::ZERO),
        );
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
        let summary_hash_present = indices
            .summary_hash_present
            .get_target(&proof.public_inputs);
        let summary_hash = indices.summary_hash.get_target(&proof.public_inputs);

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
        let l_hash = left.summary_hash;
        let r_present = right.summary_hash_present;
        let r_hash = right.summary_hash;

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
            summary_hash: HashOutTargetIndex::new(public_inputs, self.inputs.summary_hash),
        };
        debug_assert_eq!(indices, *child);

        BranchSubCircuit {
            indices,
            targets: self,
        }
    }
}

impl BranchSubCircuit {
    pub fn set_witness<F: RichField>(&self, _inputs: &mut PartialWitness<F>) {}

    pub fn set_witness_unsafe<F: RichField>(
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
    use array_util::ArrayExt;
    use itertools::chain;
    use plonky2::plonk::circuit_data::{CircuitConfig, CircuitData};
    use plonky2::plonk::proof::ProofWithPublicInputs;

    use super::*;
    use crate::subcircuits::bounded;
    use crate::test_utils::{self, hash_branch, make_hashes, C, CONFIG, D, F, ZERO_HASH};

    const LEAF_VALUES: usize = 2;
    const NON_ZERO_VALUES: [HashOut<F>; LEAF_VALUES] = make_hashes(test_utils::NON_ZERO_VALUES);

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

        pub fn prove(
            &self,
            summary_hash: Option<HashOut<F>>,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
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
            left_proof: &ProofWithPublicInputs<F, C, D>,
            right_proof: &ProofWithPublicInputs<F, C, D>,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.bounded
                .set_witness(&mut inputs, left_proof, right_proof);
            self.summarized.set_witness(&mut inputs);
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

    #[tested_fixture::tested_fixture(LEAF)]
    fn build_leaf() -> DummyLeafCircuit { DummyLeafCircuit::new(&CONFIG) }

    #[tested_fixture::tested_fixture(BRANCH_1)]
    fn build_branch_1() -> DummyBranchCircuit { DummyBranchCircuit::from_leaf(&CONFIG, &LEAF) }

    #[tested_fixture::tested_fixture(BRANCH_2)]
    fn build_branch_2() -> DummyBranchCircuit {
        DummyBranchCircuit::from_branch(&CONFIG, &BRANCH_1)
    }

    fn assert_value(proof: &ProofWithPublicInputs<F, C, D>, hash: Option<HashOut<F>>) {
        let indices = &LEAF.summarized.indices;

        let p_present = indices.summary_hash_present.get_field(&proof.public_inputs);
        assert_eq!(p_present, hash.is_some());

        let p_merged = indices.summary_hash.get_field(&proof.public_inputs);
        assert_eq!(p_merged, hash.unwrap_or_default());
    }

    #[tested_fixture::tested_fixture(EMPTY_LEAF_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_empty_leaf() -> Result<ProofWithPublicInputs<F, C, D>> {
        let proof = LEAF.prove(None)?;
        assert_value(&proof, None);
        LEAF.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(ZERO_LEAF_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_zero_leaf() -> Result<ProofWithPublicInputs<F, C, D>> {
        let proof = LEAF.prove(Some(ZERO_HASH))?;
        assert_value(&proof, Some(ZERO_HASH));
        LEAF.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(NON_ZERO_LEAF_PROOFS: [(HashOut<F>, ProofWithPublicInputs<F, C, D>); LEAF_VALUES])]
    fn verify_leaf() -> Result<[(HashOut<F>, ProofWithPublicInputs<F, C, D>); LEAF_VALUES]> {
        NON_ZERO_VALUES.try_map_ext(|non_zero_hash| {
            let proof = LEAF.prove(Some(non_zero_hash))?;
            assert_value(&proof, Some(non_zero_hash));
            LEAF.circuit.verify(proof.clone())?;
            Ok((non_zero_hash, proof))
        })
    }

    #[test]
    #[should_panic(expected = "Tried to invert zero")]
    fn bad_non_zero_leaf() {
        let proof = LEAF.prove_unsafe(false, NON_ZERO_VALUES[0]).unwrap();
        LEAF.circuit.verify(proof).unwrap();
    }

    #[tested_fixture::tested_fixture(EMPTY_BRANCH_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_empty_branch() -> Result<ProofWithPublicInputs<F, C, D>> {
        let proof = BRANCH_1.prove(&EMPTY_LEAF_PROOF, &EMPTY_LEAF_PROOF)?;
        assert_value(&proof, None);
        BRANCH_1.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(ZERO_BRANCH_PROOFS: [(HashOut<F>, ProofWithPublicInputs<F, C, D>); 2])]
    fn verify_zero_branch() -> Result<[(HashOut<F>, ProofWithPublicInputs<F, C, D>); 2]> {
        let proof_0 = BRANCH_1.prove(&EMPTY_LEAF_PROOF, &ZERO_LEAF_PROOF)?;
        assert_value(&proof_0, Some(ZERO_HASH));
        BRANCH_1.circuit.verify(proof_0.clone())?;

        let proof_1 = BRANCH_1.prove(&ZERO_LEAF_PROOF, &EMPTY_LEAF_PROOF)?;
        assert_value(&proof_1, Some(ZERO_HASH));
        BRANCH_1.circuit.verify(proof_1.clone())?;

        Ok([(ZERO_HASH, proof_0), (ZERO_HASH, proof_1)])
    }

    #[tested_fixture::tested_fixture(LEFT_BRANCH_PROOFS: [(HashOut<F>, ProofWithPublicInputs<F, C, D>); LEAF_VALUES])]
    fn verify_left_branch() -> Result<[(HashOut<F>, ProofWithPublicInputs<F, C, D>); LEAF_VALUES]> {
        NON_ZERO_LEAF_PROOFS
            .each_ref()
            .try_map_ext(|&(non_zero_hash, ref non_zero_leaf)| {
                let proof = BRANCH_1.prove(non_zero_leaf, &EMPTY_LEAF_PROOF)?;
                assert_value(&proof, Some(non_zero_hash));
                BRANCH_1.circuit.verify(proof.clone())?;
                Ok((non_zero_hash, proof))
            })
    }

    #[tested_fixture::tested_fixture(RIGHT_BRANCH_PROOFS: [(HashOut<F>, ProofWithPublicInputs<F, C, D>); LEAF_VALUES])]
    fn verify_right_branch() -> Result<[(HashOut<F>, ProofWithPublicInputs<F, C, D>); LEAF_VALUES]>
    {
        NON_ZERO_LEAF_PROOFS
            .each_ref()
            .try_map_ext(|&(non_zero_hash, ref non_zero_leaf)| {
                let proof = BRANCH_1.prove(&EMPTY_LEAF_PROOF, non_zero_leaf)?;
                assert_value(&proof, Some(non_zero_hash));
                BRANCH_1.circuit.verify(proof.clone())?;
                Ok((non_zero_hash, proof))
            })
    }

    #[tested_fixture::tested_fixture(FULL_BRANCH_PROOFS: [[(HashOut<F>, ProofWithPublicInputs<F, C, D>); LEAF_VALUES]; LEAF_VALUES])]
    fn verify_full_branch(
    ) -> Result<[[(HashOut<F>, ProofWithPublicInputs<F, C, D>); LEAF_VALUES]; LEAF_VALUES]> {
        NON_ZERO_LEAF_PROOFS
            .each_ref()
            .try_map_ext(|(non_zero_hash_1, non_zero_leaf_1)| {
                NON_ZERO_LEAF_PROOFS
                    .each_ref()
                    .try_map_ext(|(non_zero_hash_2, non_zero_leaf_2)| {
                        let both_hash = hash_branch(non_zero_hash_1, non_zero_hash_2);
                        let proof = BRANCH_1.prove(non_zero_leaf_1, non_zero_leaf_2)?;
                        assert_value(&proof, Some(both_hash));
                        BRANCH_1.circuit.verify(proof.clone())?;
                        Ok((both_hash, proof))
                    })
            })
    }

    #[tested_fixture::tested_fixture(FULL_ZERO_BRANCH_PROOFS: [[(HashOut<F>, ProofWithPublicInputs<F, C, D>); 2]; LEAF_VALUES])]
    fn verify_zero_full_branch(
    ) -> Result<[[(HashOut<F>, ProofWithPublicInputs<F, C, D>); 2]; LEAF_VALUES]> {
        NON_ZERO_LEAF_PROOFS
            .each_ref()
            .try_map_ext(|(non_zero_hash, non_zero_leaf)| {
                let hash_0 = hash_branch(&ZERO_HASH, non_zero_hash);
                let proof_0 = BRANCH_1.prove(&ZERO_LEAF_PROOF, non_zero_leaf)?;
                assert_value(&proof_0, Some(hash_0));
                BRANCH_1.circuit.verify(proof_0.clone())?;

                let hash_1 = hash_branch(&ZERO_HASH, non_zero_hash);
                let proof_1 = BRANCH_1.prove(&ZERO_LEAF_PROOF, non_zero_leaf)?;
                assert_value(&proof_1, Some(hash_1));
                BRANCH_1.circuit.verify(proof_1.clone())?;

                Ok([(hash_0, proof_0), (hash_1, proof_1)])
            })
    }

    #[test]
    #[ignore = "slow"]
    fn verify_double_branch() -> Result<()> {
        let branches = chain![
            ZERO_BRANCH_PROOFS.iter(),
            LEFT_BRANCH_PROOFS.iter(),
            RIGHT_BRANCH_PROOFS.iter(),
            FULL_BRANCH_PROOFS.iter().flatten(),
            FULL_ZERO_BRANCH_PROOFS.iter().flatten(),
        ];

        for &(hash_1, ref proof_1) in branches.clone() {
            let proof = BRANCH_2.prove(&EMPTY_BRANCH_PROOF, proof_1)?;
            assert_value(&proof, Some(hash_1));
            BRANCH_2.circuit.verify(proof)?;

            let proof = BRANCH_2.prove(proof_1, &EMPTY_BRANCH_PROOF)?;
            assert_value(&proof, Some(hash_1));
            BRANCH_2.circuit.verify(proof)?;

            for &(hash_2, ref proof_2) in branches.clone() {
                let both_hash = hash_branch(&hash_1, &hash_2);
                let proof = BRANCH_2.prove(proof_1, proof_2)?;
                assert_value(&proof, Some(both_hash));
                BRANCH_2.circuit.verify(proof)?;

                let both_hash = hash_branch(&hash_2, &hash_1);
                let proof = BRANCH_2.prove(proof_2, proof_1)?;
                assert_value(&proof, Some(both_hash));
                BRANCH_2.circuit.verify(proof)?;
            }
        }

        Ok(())
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_zero_branch() {
        let proof = BRANCH_2
            .prove_unsafe(true, ZERO_HASH, &EMPTY_BRANCH_PROOF, &EMPTY_BRANCH_PROOF)
            .unwrap();
        BRANCH_2.circuit.verify(proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_non_zero_branch() {
        let proof = BRANCH_1
            .prove_unsafe(
                false,
                NON_ZERO_LEAF_PROOFS[0].0,
                &EMPTY_LEAF_PROOF,
                &NON_ZERO_LEAF_PROOFS[0].1,
            )
            .unwrap();
        BRANCH_1.circuit.verify(proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_wrong_hash_branch() {
        let branch_proof = BRANCH_1
            .prove_unsafe(
                true,
                NON_ZERO_LEAF_PROOFS[0].0,
                &NON_ZERO_LEAF_PROOFS[0].1,
                &NON_ZERO_LEAF_PROOFS[1].1,
            )
            .unwrap();
        BRANCH_1.circuit.verify(branch_proof).unwrap();
    }
}

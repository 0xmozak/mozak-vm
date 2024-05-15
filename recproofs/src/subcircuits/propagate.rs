//! Subcircuits for recursively proving all nodes in a tree share a common value

use std::iter::zip;

use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::target::Target;
use plonky2::iop::witness::{PartialWitness, WitnessWrite};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::proof::ProofWithPublicInputsTarget;

use crate::indices::{ArrayTargetIndex, TargetIndex};

/// The indices of the public inputs of this subcircuit in any
/// `ProofWithPublicInputs`
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PublicIndices<const V: usize> {
    /// The indices of each of the elements of the common values
    pub values: ArrayTargetIndex<TargetIndex, V>,
}

pub struct SubCircuitInputs<const V: usize> {
    /// The common values
    pub values: [Target; V],
}

pub struct LeafTargets<const V: usize> {
    /// The public inputs
    pub inputs: SubCircuitInputs<V>,
}

impl<const V: usize> SubCircuitInputs<V> {
    pub fn default<F, const D: usize>(builder: &mut CircuitBuilder<F, D>) -> Self
    where
        F: RichField + Extendable<D>, {
        let values = builder.add_virtual_target_arr::<V>();
        builder.register_public_inputs(&values);
        Self { values }
    }

    #[must_use]
    pub fn build_leaf<F, const D: usize>(
        self,
        _builder: &mut CircuitBuilder<F, D>,
    ) -> LeafTargets<V>
    where
        F: RichField + Extendable<D>, {
        LeafTargets { inputs: self }
    }
}

/// The leaf subcircuit metadata. This subcircuit does basically nothing, simply
/// expressing that some values exist
pub struct LeafSubCircuit<const V: usize> {
    pub targets: LeafTargets<V>,
    pub indices: PublicIndices<V>,
}

impl<const V: usize> LeafTargets<V> {
    #[must_use]
    pub fn build(self, public_inputs: &[Target]) -> LeafSubCircuit<V> {
        let indices = PublicIndices {
            values: ArrayTargetIndex::new(public_inputs, &self.inputs.values),
        };
        LeafSubCircuit {
            targets: self,
            indices,
        }
    }
}

impl<const V: usize> LeafSubCircuit<V> {
    /// Get ready to generate a proof
    pub fn set_witness<F: RichField>(&self, inputs: &mut PartialWitness<F>, values: [F; V]) {
        inputs.set_target_arr(&self.targets.inputs.values, &values);
    }
}

pub struct BranchTargets<const V: usize> {
    /// The public inputs
    pub inputs: SubCircuitInputs<V>,

    /// The left direction
    pub left: SubCircuitInputs<V>,

    /// The right direction
    pub right: SubCircuitInputs<V>,
}

impl<const V: usize> SubCircuitInputs<V> {
    #[must_use]
    pub fn build_branch<F: RichField + Extendable<D>, const D: usize>(
        self,
        builder: &mut CircuitBuilder<F, D>,
        indices: &PublicIndices<V>,
        left_proof: &ProofWithPublicInputsTarget<D>,
        right_proof: &ProofWithPublicInputsTarget<D>,
    ) -> BranchTargets<V> {
        let l_values = indices.values.get_target(&left_proof.public_inputs);
        let r_values = indices.values.get_target(&right_proof.public_inputs);

        // Connect all the values
        for (v, (l, r)) in zip(self.values, zip(l_values, r_values)) {
            builder.connect(v, l);
            builder.connect(l, r);
        }

        BranchTargets {
            inputs: self,
            left: SubCircuitInputs { values: l_values },
            right: SubCircuitInputs { values: r_values },
        }
    }
}

/// The branch subcircuit metadata. This subcircuit proves knowledge of two
/// private subcircuit proofs, and that the public `values` of those circuits
/// are the same as the public `values` of this circuit.
pub struct BranchSubCircuit<const V: usize> {
    pub targets: BranchTargets<V>,
    pub indices: PublicIndices<V>,
}

impl<const V: usize> BranchTargets<V> {
    #[must_use]
    pub fn build(self, child: &PublicIndices<V>, public_inputs: &[Target]) -> BranchSubCircuit<V> {
        // Find the indices
        let indices = PublicIndices {
            values: ArrayTargetIndex::new(public_inputs, &self.inputs.values),
        };
        debug_assert_eq!(indices, *child);

        BranchSubCircuit {
            indices,
            targets: self,
        }
    }
}

impl<const V: usize> BranchSubCircuit<V> {
    /// Get ready to generate a proof
    pub fn set_witness<F: RichField>(&self, inputs: &mut PartialWitness<F>, values: [F; V]) {
        inputs.set_target_arr(&self.targets.inputs.values, &values);
    }
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use array_util::{try_from_fn, ArrayExt};
    use plonky2::hash::hash_types::HashOut;
    use plonky2::plonk::circuit_data::{CircuitConfig, CircuitData};
    use plonky2::plonk::proof::ProofWithPublicInputs;

    use super::*;
    use crate::subcircuits::bounded;
    use crate::test_utils::{self, make_hashes, C, CONFIG, D, F, ZERO_HASH};

    const LEAF_VALUES: usize = 2;
    const NON_ZERO_VALUES: [HashOut<F>; LEAF_VALUES] = make_hashes(test_utils::NON_ZERO_VALUES);

    pub struct DummyLeafCircuit {
        pub bounded: bounded::LeafSubCircuit,
        pub propagate: LeafSubCircuit<4>,
        pub circuit: CircuitData<F, C, D>,
    }

    impl DummyLeafCircuit {
        #[must_use]
        pub fn new(circuit_config: &CircuitConfig) -> Self {
            let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());

            let bounded_inputs = bounded::SubCircuitInputs::default(&mut builder);
            let propagate_inputs = SubCircuitInputs::default(&mut builder);

            let bounded_targets = bounded_inputs.build_leaf(&mut builder);
            let propagate_targets = propagate_inputs.build_leaf(&mut builder);

            let circuit = builder.build();

            let public_inputs = &circuit.prover_only.public_inputs;
            let bounded = bounded_targets.build(public_inputs);
            let propagate = propagate_targets.build(public_inputs);

            Self {
                bounded,
                propagate,
                circuit,
            }
        }

        pub fn prove(&self, value: [F; 4]) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.bounded.set_witness(&mut inputs);
            self.propagate.set_witness(&mut inputs, value);
            self.circuit.prove(inputs)
        }
    }

    pub struct DummyBranchCircuit {
        pub bounded: bounded::BranchSubCircuit<D>,
        pub propagate: BranchSubCircuit<4>,
        pub circuit: CircuitData<F, C, D>,
    }

    impl DummyBranchCircuit {
        #[must_use]
        pub fn new(
            circuit_config: &CircuitConfig,
            indices: &PublicIndices<4>,
            child: &CircuitData<F, C, D>,
        ) -> Self {
            let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());

            let bounded_inputs = bounded::SubCircuitInputs::default(&mut builder);
            let propagate_inputs = SubCircuitInputs::default(&mut builder);

            let bounded_targets = bounded_inputs.build_branch(&mut builder, child);
            let propagate_targets = propagate_inputs.build_branch(
                &mut builder,
                indices,
                &bounded_targets.left_proof,
                &bounded_targets.right_proof,
            );

            let circuit = builder.build();

            let public_inputs = &circuit.prover_only.public_inputs;
            let bounded = bounded_targets.build(public_inputs);
            let propagate = propagate_targets.build(indices, public_inputs);

            Self {
                bounded,
                propagate,
                circuit,
            }
        }

        #[must_use]
        pub fn from_leaf(circuit_config: &CircuitConfig, leaf: &DummyLeafCircuit) -> Self {
            Self::new(circuit_config, &leaf.propagate.indices, &leaf.circuit)
        }

        #[must_use]
        pub fn from_branch(circuit_config: &CircuitConfig, branch: &Self) -> Self {
            Self::new(circuit_config, &branch.propagate.indices, &branch.circuit)
        }

        pub fn prove(
            &self,
            value: [F; 4],
            left_proof: &ProofWithPublicInputs<F, C, D>,
            right_proof: &ProofWithPublicInputs<F, C, D>,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.bounded
                .set_witness(&mut inputs, left_proof, right_proof);
            self.propagate.set_witness(&mut inputs, value);
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

    #[tested_fixture::tested_fixture(ZERO_LEAF_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_zero_leaf() -> Result<ProofWithPublicInputs<F, C, D>> {
        let proof = LEAF.prove(ZERO_HASH.elements)?;
        LEAF.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(NON_ZERO_LEAF_PROOFS: [ProofWithPublicInputs<F, C, D>; LEAF_VALUES])]
    fn verify_leaf() -> Result<[ProofWithPublicInputs<F, C, D>; LEAF_VALUES]> {
        NON_ZERO_VALUES.try_map_ext(|non_zero_hash| {
            let proof = LEAF.prove(non_zero_hash.elements)?;
            LEAF.circuit.verify(proof.clone())?;
            Ok(proof)
        })
    }

    #[tested_fixture::tested_fixture(ZERO_BRANCH_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_zero_branch() -> Result<ProofWithPublicInputs<F, C, D>> {
        let proof = BRANCH_1.prove(ZERO_HASH.elements, &ZERO_LEAF_PROOF, &ZERO_LEAF_PROOF)?;
        BRANCH_1.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(NON_ZERO_BRANCH_PROOFS: [ProofWithPublicInputs<F, C, D>; LEAF_VALUES])]
    fn verify_branch() -> Result<[ProofWithPublicInputs<F, C, D>; LEAF_VALUES]> {
        try_from_fn(|i| {
            let proof = BRANCH_1.prove(
                NON_ZERO_VALUES[i].elements,
                &NON_ZERO_LEAF_PROOFS[i],
                &NON_ZERO_LEAF_PROOFS[i],
            )?;
            BRANCH_1.circuit.verify(proof.clone())?;
            Ok(proof)
        })
    }

    #[test]
    #[should_panic(expected = "assertion `left == right` failed")]
    fn bad_zero_branch() {
        let proof = BRANCH_1
            .prove(
                NON_ZERO_VALUES[0].elements,
                &ZERO_LEAF_PROOF,
                &ZERO_LEAF_PROOF,
            )
            .unwrap();
        BRANCH_1.circuit.verify(proof.clone()).unwrap();
    }

    #[test]
    #[should_panic(expected = "assertion `left == right` failed")]
    fn bad_non_zero_branch() {
        let proof = BRANCH_1
            .prove(
                ZERO_HASH.elements,
                &NON_ZERO_LEAF_PROOFS[0],
                &NON_ZERO_LEAF_PROOFS[0],
            )
            .unwrap();
        BRANCH_1.circuit.verify(proof.clone()).unwrap();
    }

    #[test]
    #[should_panic(expected = "assertion `left == right` failed")]
    fn bad_mismatch_branch() {
        let proof = BRANCH_1
            .prove(
                ZERO_HASH.elements,
                &ZERO_LEAF_PROOF,
                &NON_ZERO_LEAF_PROOFS[0],
            )
            .unwrap();
        BRANCH_1.circuit.verify(proof.clone()).unwrap();
    }

    #[test]
    fn verify_zero_double_branch() -> Result<()> {
        let proof = BRANCH_2.prove(ZERO_HASH.elements, &ZERO_BRANCH_PROOF, &ZERO_BRANCH_PROOF)?;
        BRANCH_2.circuit.verify(proof.clone())?;
        Ok(())
    }

    #[test]
    fn verify_double_branch() -> Result<()> {
        for i in 0..LEAF_VALUES {
            let proof = BRANCH_2.prove(
                NON_ZERO_VALUES[i].elements,
                &NON_ZERO_BRANCH_PROOFS[i],
                &NON_ZERO_BRANCH_PROOFS[i],
            )?;
            BRANCH_2.circuit.verify(proof.clone())?;
        }
        Ok(())
    }
}

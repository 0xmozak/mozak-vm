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
        let l_values = indices.values.get(&left_proof.public_inputs);
        let r_values = indices.values.get(&right_proof.public_inputs);

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
    use lazy_static::lazy_static;
    use plonky2::field::types::Field;
    use plonky2::plonk::circuit_data::{CircuitConfig, CircuitData};
    use plonky2::plonk::proof::ProofWithPublicInputs;

    use super::*;
    use crate::subcircuits::bounded;
    use crate::test_utils::{C, CONFIG, D, F};

    pub struct DummyLeafCircuit {
        pub bounded: bounded::LeafSubCircuit,
        pub propagate: LeafSubCircuit<3>,
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

        pub fn prove(&self, value: [F; 3]) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.bounded.set_witness(&mut inputs);
            self.propagate.set_witness(&mut inputs, value);
            self.circuit.prove(inputs)
        }
    }

    pub struct DummyBranchCircuit {
        pub bounded: bounded::BranchSubCircuit<D>,
        pub propagate: BranchSubCircuit<3>,
        pub circuit: CircuitData<F, C, D>,
    }

    impl DummyBranchCircuit {
        #[must_use]
        pub fn new(
            circuit_config: &CircuitConfig,
            indices: &PublicIndices<3>,
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
            value: [F; 3],
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

    lazy_static! {
        static ref LEAF: DummyLeafCircuit = DummyLeafCircuit::new(&CONFIG);
        static ref BRANCH_1: DummyBranchCircuit = DummyBranchCircuit::from_leaf(&CONFIG, &LEAF);
        static ref BRANCH_2: DummyBranchCircuit =
            DummyBranchCircuit::from_branch(&CONFIG, &BRANCH_1);
    }

    #[test]
    fn verify_leaf() -> Result<()> {
        let zero = [F::ZERO; 3];
        let non_zero = [1, 2, 99].map(F::from_canonical_u64);

        let proof = LEAF.prove(zero)?;
        LEAF.circuit.verify(proof)?;

        let proof = LEAF.prove(non_zero)?;
        LEAF.circuit.verify(proof)?;

        Ok(())
    }

    #[test]
    fn verify_branch() -> Result<()> {
        let zero = [F::ZERO; 3];
        let non_zero = [1, 2, 99].map(F::from_canonical_u64);

        // Leaf proofs
        let zero_proof = LEAF.prove(zero)?;
        LEAF.circuit.verify(zero_proof.clone())?;

        let non_zero_proof = LEAF.prove(non_zero)?;
        LEAF.circuit.verify(non_zero_proof.clone())?;

        // Branch proofs
        let branch_zero_proof = BRANCH_1.prove(zero, &zero_proof, &zero_proof)?;
        BRANCH_1.circuit.verify(branch_zero_proof.clone())?;

        let branch_non_zero_proof = BRANCH_1.prove(non_zero, &non_zero_proof, &non_zero_proof)?;
        BRANCH_1.circuit.verify(branch_non_zero_proof.clone())?;

        // Double branch proofs
        let double_branch_zero_proof =
            BRANCH_2.prove(zero, &branch_zero_proof, &branch_zero_proof)?;
        BRANCH_2.circuit.verify(double_branch_zero_proof)?;

        let double_branch_non_zero_proof =
            BRANCH_2.prove(non_zero, &branch_non_zero_proof, &branch_non_zero_proof)?;
        BRANCH_2.circuit.verify(double_branch_non_zero_proof)?;

        Ok(())
    }
}

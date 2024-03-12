//! Subcircuits for recursively proving all nodes in a tree share a common value
use std::iter::zip;

use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::target::Target;
use plonky2::iop::witness::{PartialWitness, WitnessWrite};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::proof::ProofWithPublicInputsTarget;

use super::find_targets;

/// The indices of the public inputs of this subcircuit in any
/// `ProofWithPublicInputs`
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PublicIndices<const V: usize> {
    /// The indices of each of the elements of the common values
    pub values: [usize; V],
}

impl<const V: usize> PublicIndices<V> {
    /// Extract common values from an array of public inputs.
    pub fn get_values<T: Copy>(&self, public_inputs: &[T]) -> [T; V] {
        self.values.map(|i| public_inputs[i])
    }

    /// Insert common values into an array of public inputs.
    pub fn set_values<T>(&self, public_inputs: &mut [T], v: [T; V]) {
        for (i, v) in v.into_iter().enumerate() {
            public_inputs[self.values[i]] = v;
        }
    }
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
    pub fn build<F, const D: usize>(self, _builder: &mut CircuitBuilder<F, D>) -> LeafTargets<V>
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
    pub fn build_leaf(self, public_inputs: &[Target]) -> LeafSubCircuit<V> {
        let indices = PublicIndices {
            values: find_targets(public_inputs, self.inputs.values),
        };
        LeafSubCircuit {
            targets: self,
            indices,
        }
    }
}

impl<const V: usize> LeafSubCircuit<V> {
    /// Get ready to generate a proof
    pub fn set_inputs<F: RichField>(&self, inputs: &mut PartialWitness<F>, values: [F; V]) {
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
    fn direction_from_node<const D: usize>(
        proof: &ProofWithPublicInputsTarget<D>,
        indices: &PublicIndices<V>,
    ) -> SubCircuitInputs<V> {
        let values = indices.get_values(&proof.public_inputs);

        SubCircuitInputs { values }
    }

    fn build_helper<F: RichField + Extendable<D>, const D: usize>(
        self,
        builder: &mut CircuitBuilder<F, D>,
        left: SubCircuitInputs<V>,
        right: SubCircuitInputs<V>,
    ) -> BranchTargets<V> {
        // Connect all the values
        for (v, (l, r)) in zip(self.values, zip(left.values, right.values)) {
            builder.connect(v, l);
            builder.connect(l, r);
        }

        BranchTargets {
            inputs: self,
            left,
            right,
        }
    }

    #[must_use]
    pub fn from_leaf<F: RichField + Extendable<D>, const D: usize>(
        self,
        builder: &mut CircuitBuilder<F, D>,
        leaf: &LeafSubCircuit<V>,
        left_proof: &ProofWithPublicInputsTarget<D>,
        right_proof: &ProofWithPublicInputsTarget<D>,
    ) -> BranchTargets<V> {
        let left = Self::direction_from_node(left_proof, &leaf.indices);
        let right = Self::direction_from_node(right_proof, &leaf.indices);
        self.build_helper(builder, left, right)
    }

    pub fn from_branch<F: RichField + Extendable<D>, const D: usize>(
        self,
        builder: &mut CircuitBuilder<F, D>,
        branch: &BranchSubCircuit<V>,
        left_proof: &ProofWithPublicInputsTarget<D>,
        right_proof: &ProofWithPublicInputsTarget<D>,
    ) -> BranchTargets<V> {
        let left = Self::direction_from_node(left_proof, &branch.indices);
        let right = Self::direction_from_node(right_proof, &branch.indices);
        self.build_helper(builder, left, right)
    }
}

/// The branch subcircuit metadata. This subcircuit proves knowledge of two
/// private subcircuit proofs, and that the public `values` of those circuits
/// are the same as the public `values` of this circuit.
pub struct BranchSubCircuit<const V: usize> {
    pub targets: BranchTargets<V>,
    pub indices: PublicIndices<V>,
    /// The distance from the leaves (`0` being the lowest branch)
    /// Used for debugging
    pub dbg_height: usize,
}

impl<const V: usize> BranchTargets<V> {
    fn get_indices(&self, public_inputs: &[Target]) -> PublicIndices<V> {
        PublicIndices {
            values: find_targets(public_inputs, self.inputs.values),
        }
    }

    #[must_use]
    pub fn from_leaf(self, public_inputs: &[Target]) -> BranchSubCircuit<V> {
        BranchSubCircuit {
            indices: self.get_indices(public_inputs),
            targets: self,
            dbg_height: 0,
        }
    }

    #[must_use]
    pub fn from_branch(
        self,
        branch: &BranchSubCircuit<V>,
        public_inputs: &[Target],
    ) -> BranchSubCircuit<V> {
        BranchSubCircuit {
            indices: self.get_indices(public_inputs),
            targets: self,
            dbg_height: branch.dbg_height + 1,
        }
    }
}

impl<const V: usize> BranchSubCircuit<V> {
    /// Get ready to generate a proof
    pub fn set_inputs<F: RichField>(&self, inputs: &mut PartialWitness<F>, values: [F; V]) {
        inputs.set_target_arr(&self.targets.inputs.values, &values);
    }
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use plonky2::field::types::Field;
    use plonky2::plonk::circuit_data::{CircuitConfig, CircuitData};
    use plonky2::plonk::proof::ProofWithPublicInputs;

    use super::*;
    use crate::test_utils::{C, D, F};

    pub struct DummyLeafCircuit {
        pub propagate: LeafSubCircuit<3>,
        pub circuit: CircuitData<F, C, D>,
    }

    impl DummyLeafCircuit {
        #[must_use]
        pub fn new(circuit_config: &CircuitConfig) -> Self {
            let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());

            let propagate_inputs = SubCircuitInputs::default(&mut builder);
            let propagate_targets = propagate_inputs.build(&mut builder);
            let circuit = builder.build();
            let propagate = propagate_targets.build_leaf(&circuit.prover_only.public_inputs);

            Self { propagate, circuit }
        }

        pub fn prove(&self, value: [F; 3]) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.propagate.set_inputs(&mut inputs, value);
            self.circuit.prove(inputs)
        }
    }

    pub struct DummyBranchCircuit {
        pub propagate: BranchSubCircuit<3>,
        pub circuit: CircuitData<F, C, D>,
        pub targets: DummyBranchTargets,
    }

    pub struct DummyBranchTargets {
        pub left_proof: ProofWithPublicInputsTarget<D>,
        pub right_proof: ProofWithPublicInputsTarget<D>,
    }

    impl DummyBranchCircuit {
        #[must_use]
        pub fn from_leaf(circuit_config: &CircuitConfig, leaf: &DummyLeafCircuit) -> Self {
            let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());

            let circuit_data = &leaf.circuit;
            let common = &circuit_data.common;
            let verifier = builder.constant_verifier_data(&circuit_data.verifier_only);
            let left_proof = builder.add_virtual_proof_with_pis(common);
            let right_proof = builder.add_virtual_proof_with_pis(common);

            let propagate_inputs = SubCircuitInputs::default(&mut builder);

            builder.verify_proof::<C>(&left_proof, &verifier, common);
            builder.verify_proof::<C>(&right_proof, &verifier, common);
            let propagate_targets = propagate_inputs.from_leaf(
                &mut builder,
                &leaf.propagate,
                &left_proof,
                &right_proof,
            );
            let targets = DummyBranchTargets {
                left_proof,
                right_proof,
            };

            let circuit = builder.build();
            let propagate = propagate_targets.from_leaf(&circuit.prover_only.public_inputs);

            Self {
                propagate,
                circuit,
                targets,
            }
        }

        pub fn from_branch(circuit_config: &CircuitConfig, branch: &DummyBranchCircuit) -> Self {
            let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());

            let circuit_data = &branch.circuit;
            let common = &circuit_data.common;
            let verifier = builder.constant_verifier_data(&circuit_data.verifier_only);
            let left_proof = builder.add_virtual_proof_with_pis(common);
            let right_proof = builder.add_virtual_proof_with_pis(common);
            let propagate_inputs = SubCircuitInputs::default(&mut builder);

            builder.verify_proof::<C>(&left_proof, &verifier, common);
            builder.verify_proof::<C>(&right_proof, &verifier, common);
            let propagate_targets = propagate_inputs.from_branch(
                &mut builder,
                &branch.propagate,
                &left_proof,
                &right_proof,
            );
            let targets = DummyBranchTargets {
                left_proof,
                right_proof,
            };

            let circuit = builder.build();
            let propagate = propagate_targets
                .from_branch(&branch.propagate, &circuit.prover_only.public_inputs);

            Self {
                propagate,
                circuit,
                targets,
            }
        }

        pub fn prove(
            &self,
            left_proof: &ProofWithPublicInputs<F, C, D>,
            right_proof: &ProofWithPublicInputs<F, C, D>,
            value: [F; 3],
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            inputs.set_proof_with_pis_target(&self.targets.left_proof, left_proof);
            inputs.set_proof_with_pis_target(&self.targets.right_proof, right_proof);
            self.propagate.set_inputs(&mut inputs, value);
            self.circuit.prove(inputs)
        }
    }

    #[test]
    fn verify_leaf() -> Result<()> {
        let circuit_config = CircuitConfig::standard_recursion_config();
        let circuit = DummyLeafCircuit::new(&circuit_config);

        let zero = [F::ZERO; 3];
        let non_zero = [1, 2, 99].map(F::from_canonical_u64);

        let proof = circuit.prove(zero)?;
        circuit.circuit.verify(proof)?;

        let proof = circuit.prove(non_zero)?;
        circuit.circuit.verify(proof)?;

        Ok(())
    }

    #[test]
    fn verify_branch() -> Result<()> {
        let circuit_config = CircuitConfig::standard_recursion_config();
        let leaf_circuit = DummyLeafCircuit::new(&circuit_config);
        let branch_circuit_1 = DummyBranchCircuit::from_leaf(&circuit_config, &leaf_circuit);
        let branch_circuit_2 = DummyBranchCircuit::from_branch(&circuit_config, &branch_circuit_1);

        let zero = [F::ZERO; 3];
        let non_zero = [1, 2, 99].map(F::from_canonical_u64);

        // Leaf proofs
        let zero_proof = leaf_circuit.prove(zero)?;
        leaf_circuit.circuit.verify(zero_proof.clone())?;

        let non_zero_proof = leaf_circuit.prove(non_zero)?;
        leaf_circuit.circuit.verify(non_zero_proof.clone())?;

        // Branch proofs
        let branch_zero_proof = branch_circuit_1.prove(&zero_proof, &zero_proof, zero)?;
        branch_circuit_1.circuit.verify(branch_zero_proof.clone())?;

        let branch_non_zero_proof =
            branch_circuit_1.prove(&non_zero_proof, &non_zero_proof, non_zero)?;
        branch_circuit_1
            .circuit
            .verify(branch_non_zero_proof.clone())?;

        // Double branch proofs
        let double_branch_zero_proof =
            branch_circuit_2.prove(&branch_zero_proof, &branch_zero_proof, zero)?;
        branch_circuit_2.circuit.verify(double_branch_zero_proof)?;

        let double_branch_non_zero_proof =
            branch_circuit_2.prove(&branch_non_zero_proof, &branch_non_zero_proof, non_zero)?;
        branch_circuit_2
            .circuit
            .verify(double_branch_non_zero_proof)?;

        Ok(())
    }
}

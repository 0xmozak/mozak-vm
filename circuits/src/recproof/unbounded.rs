//! Subcircuits for recursively proving an unbounded binary tree.
//!
//! These subcircuits are fully-recursive, building on top of each other to
//! create the next level up of the tree.
use std::array;

use plonky2::field::extension::Extendable;
use plonky2::gates::noop::NoopGate;
use plonky2::hash::hash_types::{HashOutTarget, MerkleCapTarget, RichField, NUM_HASH_OUT_ELTS};
use plonky2::iop::target::{BoolTarget, Target};
use plonky2::iop::witness::{PartialWitness, WitnessWrite};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::{
    CircuitConfig, CircuitData, CommonCircuitData, VerifierCircuitTarget,
};
use plonky2::plonk::config::{AlgebraicHasher, GenericConfig};
use plonky2::plonk::proof::ProofWithPublicInputsTarget;

use super::select_verifier;
use crate::stark::recursive_verifier::{
    circuit_data_for_recursion, FINAL_RECURSION_THRESHOLD_DEGREE_BITS,
};

fn from_slice<F: RichField + Extendable<D>, const D: usize>(
    slice: &[Target],
    common_data: &CommonCircuitData<F, D>,
) -> VerifierCircuitTarget {
    let cap_len = common_data.config.fri_config.num_cap_elements();
    let len = slice.len();
    let constants_sigmas_cap = MerkleCapTarget(
        (0..cap_len)
            .map(|i| HashOutTarget {
                elements: array::from_fn(|j| slice[len - NUM_HASH_OUT_ELTS * (cap_len - i) + j]),
            })
            .collect(),
    );
    let circuit_digest = HashOutTarget {
        elements: array::from_fn(|i| {
            slice[len - NUM_HASH_OUT_ELTS - NUM_HASH_OUT_ELTS * cap_len + i]
        }),
    };

    VerifierCircuitTarget {
        constants_sigmas_cap,
        circuit_digest,
    }
}

pub struct Targets {
    pub verifier_data_target: VerifierCircuitTarget,
}

/// The leaf subcircuit metadata. This subcircuit does basically nothing, simply
/// expressing a verifier to bind the tree's recursion
pub struct LeafSubCircuit {
    pub targets: Targets,
}

impl LeafSubCircuit {
    #[must_use]
    pub fn new<F, C, const D: usize>(
        mut builder: CircuitBuilder<F, D>,
    ) -> (CircuitData<F, C, D>, Self)
    where
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F>,
        C::Hasher: AlgebraicHasher<F>, {
        let mut common_data = circuit_data_for_recursion::<F, C, D>(
            &CircuitConfig::standard_recursion_config(),
            FINAL_RECURSION_THRESHOLD_DEGREE_BITS,
            0,
        )
        .common;
        let verifier_data_target = builder.add_verifier_data_public_inputs();
        common_data.num_public_inputs = builder.num_public_inputs();

        // Make sure we have enough gates to match `common_data`.
        while builder.num_gates() < (common_data.degree() / 2) {
            builder.add_gate(NoopGate, vec![]);
        }
        // Make sure we have every gate to match `common_data`.
        for g in &common_data.gates {
            builder.add_gate_to_gate_set(g.clone());
        }

        let targets = Targets {
            verifier_data_target,
        };

        // Build the circuit
        (builder.build(), Self { targets })
    }

    /// Get ready to generate a proof
    pub fn set_inputs<F, C, const D: usize>(
        &self,
        inputs: &mut PartialWitness<F>,
        branch: &CircuitData<F, C, D>,
    ) where
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F>,
        C::Hasher: AlgebraicHasher<F>, {
        inputs.set_verifier_data_target(&self.targets.verifier_data_target, &branch.verifier_only);
    }
}

/// The branch subcircuit metadata. This subcircuit proves knowledge of two
/// private subcircuit proofs, all bound to use the same recursive verifier.
pub struct BranchSubCircuit {
    pub targets: Targets,
}

impl BranchSubCircuit {
    #[must_use]
    pub fn new<F, C, const D: usize>(
        mut builder: CircuitBuilder<F, D>,
        leaf: &CircuitData<F, C, D>,
        left_is_leaf: BoolTarget,
        right_is_leaf: BoolTarget,
        left_proof: &ProofWithPublicInputsTarget<D>,
        right_proof: &ProofWithPublicInputsTarget<D>,
    ) -> (CircuitData<F, C, D>, Self)
    where
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F>,
        C::Hasher: AlgebraicHasher<F>, {
        let leaf_verifier = builder.constant_verifier_data(&leaf.verifier_only);
        let verifier_data_target = builder.add_verifier_data_public_inputs();

        // Connect previous verifier data to current one. This guarantees that every
        // proof in the cycle uses the same verifier data.
        let left_verifier = from_slice::<F, D>(&left_proof.public_inputs, &leaf.common);
        builder.connect_hashes(
            left_verifier.circuit_digest,
            verifier_data_target.circuit_digest,
        );
        builder.connect_merkle_caps(
            &left_verifier.constants_sigmas_cap,
            &verifier_data_target.constants_sigmas_cap,
        );
        let right_verifier = from_slice::<F, D>(&left_proof.public_inputs, &leaf.common);
        builder.connect_hashes(
            right_verifier.circuit_digest,
            verifier_data_target.circuit_digest,
        );
        builder.connect_merkle_caps(
            &right_verifier.constants_sigmas_cap,
            &verifier_data_target.constants_sigmas_cap,
        );

        let left_verifier = select_verifier(
            &mut builder,
            left_is_leaf,
            &leaf_verifier,
            &verifier_data_target,
        );
        let right_verifier = select_verifier(
            &mut builder,
            right_is_leaf,
            &leaf_verifier,
            &verifier_data_target,
        );
        builder.verify_proof::<C>(left_proof, &left_verifier, &leaf.common);
        builder.verify_proof::<C>(right_proof, &right_verifier, &leaf.common);

        // Make sure we have enough gates to match `common_data`.
        while builder.num_gates() < (leaf.common.degree() / 2) {
            builder.add_gate(NoopGate, vec![]);
        }

        // Make sure we have every gate to match `common_data`.
        for g in &leaf.common.gates {
            builder.add_gate_to_gate_set(g.clone());
        }
        let targets = Targets {
            verifier_data_target,
        };

        // Build the circuit
        (builder.build(), Self { targets })
    }
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use plonky2::plonk::circuit_data::CircuitConfig;
    use plonky2::plonk::proof::ProofWithPublicInputs;

    use super::*;
    use crate::test_utils::{C, D, F};

    pub struct DummyLeafCircuit {
        pub unbounded: LeafSubCircuit,
        pub circuit: CircuitData<F, C, D>,
    }

    impl DummyLeafCircuit {
        #[must_use]
        pub fn new(circuit_config: &CircuitConfig) -> Self {
            let builder = CircuitBuilder::<F, D>::new(circuit_config.clone());
            let (circuit, unbounded) = LeafSubCircuit::new(builder);

            Self { unbounded, circuit }
        }

        pub fn prove(&self, branch: &DummyBranchCircuit) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.unbounded.set_inputs(&mut inputs, &branch.circuit);
            self.circuit.prove(inputs)
        }
    }

    pub struct DummyBranchCircuit {
        pub unbounded: BranchSubCircuit,
        pub circuit: CircuitData<F, C, D>,
        pub targets: DummyBranchTargets,
    }

    pub struct DummyBranchTargets {
        pub left_is_leaf: BoolTarget,
        pub right_is_leaf: BoolTarget,
        pub left_proof: ProofWithPublicInputsTarget<D>,
        pub right_proof: ProofWithPublicInputsTarget<D>,
    }

    impl DummyBranchCircuit {
        #[must_use]
        pub fn new(circuit_config: &CircuitConfig, leaf: &DummyLeafCircuit) -> Self {
            let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());

            let common = &leaf.circuit.common;
            let left_is_leaf = builder.add_virtual_bool_target_safe();
            let right_is_leaf = builder.add_virtual_bool_target_safe();
            let left_proof = builder.add_virtual_proof_with_pis(common);
            let right_proof = builder.add_virtual_proof_with_pis(common);

            let (circuit, unbounded) = BranchSubCircuit::new(
                builder,
                &leaf.circuit,
                left_is_leaf,
                right_is_leaf,
                &left_proof,
                &right_proof,
            );

            let targets = DummyBranchTargets {
                left_is_leaf,
                right_is_leaf,
                left_proof,
                right_proof,
            };

            Self {
                unbounded,
                circuit,
                targets,
            }
        }

        pub fn prove(
            &self,
            left_is_leaf: bool,
            right_is_leaf: bool,
            left_proof: &ProofWithPublicInputs<F, C, D>,
            right_proof: &ProofWithPublicInputs<F, C, D>,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            inputs.set_bool_target(self.targets.left_is_leaf, left_is_leaf);
            inputs.set_bool_target(self.targets.right_is_leaf, right_is_leaf);
            inputs.set_proof_with_pis_target(&self.targets.left_proof, left_proof);
            inputs.set_proof_with_pis_target(&self.targets.right_proof, right_proof);
            self.circuit.prove(inputs)
        }
    }

    #[test]
    fn verify_leaf() -> Result<()> {
        let circuit_config = CircuitConfig::standard_recursion_config();
        let leaf = DummyLeafCircuit::new(&circuit_config);
        let branch = DummyBranchCircuit::new(&circuit_config, &leaf);

        let proof = leaf.prove(&branch)?;
        leaf.circuit.verify(proof)?;

        Ok(())
    }

    #[test]
    fn verify_branch() -> Result<()> {
        let circuit_config = CircuitConfig::standard_recursion_config();
        let leaf = DummyLeafCircuit::new(&circuit_config);
        let branch = DummyBranchCircuit::new(&circuit_config, &leaf);

        let leaf_proof = leaf.prove(&branch)?;
        leaf.circuit.verify(leaf_proof.clone())?;

        let branch_proof_1 = branch.prove(true, true, &leaf_proof, &leaf_proof)?;
        branch.circuit.verify(branch_proof_1.clone())?;

        let branch_proof_2 = branch.prove(true, false, &leaf_proof, &branch_proof_1)?;
        branch.circuit.verify(branch_proof_2.clone())?;

        let branch_proof_3 = branch.prove(false, false, &branch_proof_1, &branch_proof_2)?;
        branch.circuit.verify(branch_proof_3)?;

        Ok(())
    }
}

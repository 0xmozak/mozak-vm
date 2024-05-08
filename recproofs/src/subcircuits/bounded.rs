//! Subcircuits for recursively proving an bounded binary tree.
//!
//! These subcircuits are pseudo-recursive, meaning the each `BranchCircuits`
//! corresponds to a specific tree height and can only be used for that height
//! (a ten layer tree needs 10 circuits).
//!
//! One advantage of this approach (in addition to just being directly faster)
//! is that since each circuit is unique based on height, the height is
//! automatically attested to

use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::target::Target;
use plonky2::iop::witness::{PartialWitness, WitnessWrite};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::CircuitData;
use plonky2::plonk::config::{AlgebraicHasher, GenericConfig};
use plonky2::plonk::proof::{ProofWithPublicInputs, ProofWithPublicInputsTarget};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct PublicIndices;

pub struct SubCircuitInputs;

pub struct LeafTargets {
    /// The public inputs
    pub inputs: SubCircuitInputs,
}

impl SubCircuitInputs {
    pub fn default<F, const D: usize>(_builder: &mut CircuitBuilder<F, D>) -> Self
    where
        F: RichField + Extendable<D>, {
        Self
    }

    #[must_use]
    pub fn build_leaf<F, const D: usize>(self, _builder: &mut CircuitBuilder<F, D>) -> LeafTargets
    where
        F: RichField + Extendable<D>, {
        LeafTargets { inputs: self }
    }
}

/// The leaf subcircuit metadata. This subcircuit does basically nothing and
/// exists simply for common API usage
pub struct LeafSubCircuit {
    pub targets: LeafTargets,
    pub indices: PublicIndices,
}

impl LeafTargets {
    #[must_use]
    pub fn build(self, _public_inputs: &[Target]) -> LeafSubCircuit {
        let indices = PublicIndices;
        LeafSubCircuit {
            targets: self,
            indices,
        }
    }
}

impl LeafSubCircuit {
    /// Get ready to generate a proof
    pub fn set_witness<F: RichField>(&self, _inputs: &mut PartialWitness<F>) {}
}

pub struct BranchTargets<const D: usize> {
    /// The public inputs
    pub inputs: SubCircuitInputs,

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
        circuit: &CircuitData<F, C, D>,
    ) -> BranchTargets<D>
    where
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F>,
        C::Hasher: AlgebraicHasher<F>, {
        let common = &circuit.common;
        let left_proof = builder.add_virtual_proof_with_pis(common);
        let right_proof = builder.add_virtual_proof_with_pis(common);
        let verifier = builder.constant_verifier_data(&circuit.verifier_only);

        builder.verify_proof::<C>(&left_proof, &verifier, common);
        builder.verify_proof::<C>(&right_proof, &verifier, common);
        BranchTargets {
            inputs: self,
            left_proof,
            right_proof,
        }
    }
}

/// The branch subcircuit metadata. This subcircuit proves knowledge of two
/// private subcircuit proofs.
pub struct BranchSubCircuit<const D: usize> {
    pub targets: BranchTargets<D>,
    pub indices: PublicIndices,
}

impl<const D: usize> BranchTargets<D> {
    #[must_use]
    pub fn build(self, _public_inputs: &[Target]) -> BranchSubCircuit<D> {
        // Find the indices
        let indices = PublicIndices;

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
    use plonky2::plonk::circuit_data::CircuitConfig;

    use super::*;
    use crate::test_utils::{C, CONFIG, D, F};

    pub struct DummyLeafCircuit {
        pub bounded: LeafSubCircuit,
        pub circuit: CircuitData<F, C, D>,
    }

    impl DummyLeafCircuit {
        #[must_use]
        pub fn new(circuit_config: &CircuitConfig) -> Self {
            let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());
            let bounded_inputs = SubCircuitInputs::default(&mut builder);

            let bounded_targets = bounded_inputs.build_leaf(&mut builder);

            let circuit = builder.build();

            let bounded = bounded_targets.build(&circuit.prover_only.public_inputs);

            Self { bounded, circuit }
        }

        pub fn prove(&self) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.bounded.set_witness(&mut inputs);
            self.circuit.prove(inputs)
        }
    }

    pub struct DummyBranchCircuit {
        pub bounded: BranchSubCircuit<D>,
        pub circuit: CircuitData<F, C, D>,
    }

    impl DummyBranchCircuit {
        #[must_use]
        fn new(circuit_config: &CircuitConfig, child: &CircuitData<F, C, D>) -> Self {
            let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());
            let bounded_inputs = SubCircuitInputs::default(&mut builder);

            let bounded_targets = bounded_inputs.build_branch(&mut builder, child);

            let circuit = builder.build();

            let bounded = bounded_targets.build(&circuit.prover_only.public_inputs);

            Self { bounded, circuit }
        }

        #[must_use]
        pub fn from_leaf(circuit_config: &CircuitConfig, leaf: &DummyLeafCircuit) -> Self {
            Self::new(circuit_config, &leaf.circuit)
        }

        #[must_use]
        pub fn from_branch(circuit_config: &CircuitConfig, branch: &Self) -> Self {
            Self::new(circuit_config, &branch.circuit)
        }

        pub fn prove(
            &self,
            left_proof: &ProofWithPublicInputs<F, C, D>,
            right_proof: &ProofWithPublicInputs<F, C, D>,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.bounded
                .set_witness(&mut inputs, left_proof, right_proof);
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

    #[tested_fixture::tested_fixture(LEAF_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_leaf() -> Result<ProofWithPublicInputs<F, C, D>> {
        let proof = LEAF.prove()?;
        LEAF.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(BRANCH_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_branch() -> Result<ProofWithPublicInputs<F, C, D>> {
        let proof = BRANCH_1.prove(&LEAF_PROOF, &LEAF_PROOF)?;
        BRANCH_1.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[test]
    fn verify_double_branch() -> Result<()> {
        let branch_proof_2 = BRANCH_2.prove(&BRANCH_PROOF, &BRANCH_PROOF)?;
        BRANCH_2.circuit.verify(branch_proof_2.clone())?;
        Ok(())
    }
}

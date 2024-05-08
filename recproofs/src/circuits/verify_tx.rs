//! Circuits for proving events correspond to a proof

use anyhow::Result;
use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::witness::PartialWitness;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::{CircuitConfig, CircuitData};
use plonky2::plonk::config::{AlgebraicHasher, GenericConfig};
use plonky2::plonk::proof::ProofWithPublicInputs;

use super::{merge, verify_program};
use crate::subcircuits::unbounded;

pub mod core;

pub struct LeafCircuit<F, C, const D: usize>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>, {
    /// The recursion subcircuit
    pub unbounded: unbounded::LeafSubCircuit,

    // The events list
    pub events: merge::embed::LeafSubCircuit,

    /// The program verifier
    pub program_verifier: core::ProgramVerifierSubCircuit<D>,

    pub circuit: CircuitData<F, C, D>,
}

impl<F, C, const D: usize> LeafCircuit<F, C, D>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    <C as GenericConfig<D>>::Hasher: AlgebraicHasher<F>,
{
    #[must_use]
    pub fn new(
        circuit_config: &CircuitConfig,
        program: &verify_program::BranchCircuit<F, C, D>,
    ) -> Self {
        let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());

        let unbounded_inputs = unbounded::SubCircuitInputs::default(&mut builder);
        let events_inputs = merge::embed::SubCircuitInputs::default(&mut builder);

        let unbounded_targets = unbounded_inputs.build_leaf::<F, C, D>(&mut builder);
        let events_targets = events_inputs.build_leaf::<F, D>(&mut builder);

        let program_verifier_targets =
            core::ProgramSetVerifierTargets::build_targets(&mut builder, program);

        // Connect the proof to the recursion
        builder.connect_hashes(
            events_targets.inputs.hash,
            program_verifier_targets.event_root,
        );
        builder.connect(
            events_targets.inputs.hash_present.target,
            program_verifier_targets.events_present.target,
        );

        let circuit = builder.build();

        let public_inputs = &circuit.prover_only.public_inputs;
        let unbounded = unbounded_targets.build(public_inputs);
        let events = events_targets.build(public_inputs);
        let program_verifier = program_verifier_targets.build(public_inputs);

        Self {
            unbounded,
            events,
            program_verifier,
            circuit,
        }
    }

    pub fn prove(
        &self,
        branch: &BranchCircuit<F, C, D>,
        program_set_proof: &ProofWithPublicInputs<F, C, D>,
    ) -> Result<ProofWithPublicInputs<F, C, D>> {
        let mut inputs = PartialWitness::new();
        self.unbounded.set_witness(&mut inputs, &branch.circuit);
        self.program_verifier
            .set_witness(&mut inputs, program_set_proof);
        self.circuit.prove(inputs)
    }
}

pub struct BranchCircuit<F, C, const D: usize>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>, {
    /// The recursion subcircuit
    pub unbounded: unbounded::BranchSubCircuit<D>,

    // The events list
    pub events: merge::embed::BranchSubCircuit<D>,

    pub circuit: CircuitData<F, C, D>,
}

impl<F, C, const D: usize> BranchCircuit<F, C, D>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    <C as GenericConfig<D>>::Hasher: AlgebraicHasher<F>,
{
    #[must_use]
    pub fn new(
        circuit_config: &CircuitConfig,
        mc: &merge::BranchCircuit<F, C, D>,
        leaf: &LeafCircuit<F, C, D>,
    ) -> Self {
        let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());

        let unbounded_inputs = unbounded::SubCircuitInputs::default(&mut builder);
        let events_inputs = merge::embed::SubCircuitInputs::default(&mut builder);

        let unbounded_targets =
            unbounded_inputs.build_branch(&mut builder, &leaf.unbounded, &leaf.circuit);
        let events_targets = events_inputs.build_branch(
            &mut builder,
            mc,
            &leaf.events.indices,
            &unbounded_targets.left_proof,
            &unbounded_targets.right_proof,
        );

        let circuit = builder.build();

        let public_inputs = &circuit.prover_only.public_inputs;
        let unbounded = unbounded_targets.build(&leaf.unbounded, public_inputs);
        let events = events_targets.build(&leaf.events.indices, public_inputs);

        Self {
            unbounded,
            events,
            circuit,
        }
    }

    /// `hash` `vm_hash` and `event_owner` only need to be provided to check
    /// externally, otherwise they will be calculated
    pub fn prove(
        &self,
        merge: &ProofWithPublicInputs<F, C, D>,
        left_is_leaf: bool,
        left_proof: &ProofWithPublicInputs<F, C, D>,
        right_is_leaf: bool,
        right_proof: &ProofWithPublicInputs<F, C, D>,
    ) -> Result<ProofWithPublicInputs<F, C, D>> {
        let mut inputs = PartialWitness::new();
        self.unbounded.set_witness(
            &mut inputs,
            left_is_leaf,
            left_proof,
            right_is_leaf,
            right_proof,
        );
        self.events.set_witness(&mut inputs, merge);
        self.circuit.prove(inputs)
    }
}

#[cfg(test)]
pub mod test {
    use super::*;
    use crate::circuits::merge::test as merge;
    use crate::circuits::verify_program::test as verify_program;
    use crate::test_utils::{C, CONFIG, D, F};

    #[tested_fixture::tested_fixture(pub LEAF)]
    fn build_leaf() -> LeafCircuit<F, C, D> { LeafCircuit::new(&CONFIG, &verify_program::BRANCH) }

    #[tested_fixture::tested_fixture(pub BRANCH)]
    fn build_branch() -> BranchCircuit<F, C, D> {
        BranchCircuit::new(&CONFIG, &merge::BRANCH, &LEAF)
    }

    #[tested_fixture::tested_fixture(pub T0_LEAF_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_t0_leaf() -> Result<ProofWithPublicInputs<F, C, D>> {
        let proof = LEAF.prove(&BRANCH, &verify_program::T0_BRANCH_PROOF)?;
        LEAF.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(pub T1_LEAF_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_t1_leaf() -> Result<ProofWithPublicInputs<F, C, D>> {
        let proof = LEAF.prove(&BRANCH, &verify_program::T1_BRANCH_PROOF)?;
        LEAF.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(pub BRANCH_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_branch() -> Result<ProofWithPublicInputs<F, C, D>> {
        let proof = BRANCH.prove(
            &merge::T0_T1_BRANCH_PROOF,
            true,
            &T0_LEAF_PROOF,
            true,
            &T1_LEAF_PROOF,
        )?;
        BRANCH.circuit.verify(proof.clone())?;
        Ok(proof)
    }
}

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
    use lazy_static::lazy_static;
    use plonky2::plonk::circuit_data::VerifierOnlyCircuitData;

    use super::*;
    use crate::circuits::merge::test::BRANCH as MERGE_BRANCH;
    use crate::circuits::verify_program::test::{
        merge_hashes, merge_merges, p1_p2 as vp_p1_p2, p2_p1 as vp_p2_p1, BRANCH as VP_BRANCH,
        LEAF as VP_LEAF, P1_BUILT_EVENTS, P2_BUILT_EVENTS, PROGRAM_1, PROGRAM_2,
    };
    use crate::test_utils::{C, CONFIG, D, F};

    lazy_static! {
        pub static ref LEAF: LeafCircuit<F, C, D> = LeafCircuit::new(&CONFIG, &VP_BRANCH,);
        pub static ref BRANCH: BranchCircuit<F, C, D> =
            BranchCircuit::new(&CONFIG, &MERGE_BRANCH, &LEAF);
        // This is not how you would do an actual merge as it doesn't intersplice
        // the trees based on address, but for testing all we require is
        // some kind of merge occurs
        pub static ref MERGE_PROOF: ProofWithPublicInputs<F, C, D> = merge_merges(
            true,
            &merge_hashes(Some(*vp_p1_p2::MERGE_HASH), None),
            true,
            &merge_hashes(None, Some(*vp_p2_p1::MERGE_HASH)),
        );
    }

    fn build_verified_program_leaf(
        verifier: &VerifierOnlyCircuitData<C, D>,
        program_proof: &ProofWithPublicInputs<F, C, D>,
        event_proof: &ProofWithPublicInputs<F, C, D>,
    ) -> ProofWithPublicInputs<F, C, D> {
        let proof = VP_LEAF
            .prove(&VP_BRANCH, verifier, program_proof, event_proof)
            .unwrap();
        VP_LEAF.circuit.verify(proof.clone()).unwrap();
        proof
    }

    fn build_verified_program_branch(
        merge_proof: &ProofWithPublicInputs<F, C, D>,
        left_is_leaf: bool,
        left: &ProofWithPublicInputs<F, C, D>,
        right_is_leaf: bool,
        right: &ProofWithPublicInputs<F, C, D>,
    ) -> ProofWithPublicInputs<F, C, D> {
        let proof = VP_BRANCH
            .prove(merge_proof, left_is_leaf, left, right_is_leaf, right)
            .unwrap();
        VP_BRANCH.circuit.verify(proof.clone()).unwrap();
        proof
    }

    pub mod p1_p2 {
        use vp_p1_p2::{MERGE_PROOF, PROGRAM_1_PROOF, PROGRAM_2_PROOF};

        use super::*;

        lazy_static! {
            pub static ref VP_1_PROOF: ProofWithPublicInputs<F, C, D> = build_verified_program_leaf(
                &PROGRAM_1.circuit.verifier_only,
                &PROGRAM_1_PROOF,
                &P1_BUILT_EVENTS.proof,
            );
            pub static ref VP_2_PROOF: ProofWithPublicInputs<F, C, D> = build_verified_program_leaf(
                &PROGRAM_2.circuit.verifier_only,
                &PROGRAM_2_PROOF,
                &P2_BUILT_EVENTS.proof,
            );
            pub static ref VP_MERGE_PROOF: ProofWithPublicInputs<F, C, D> =
                build_verified_program_branch(&MERGE_PROOF, true, &VP_1_PROOF, true, &VP_2_PROOF,);
        }
    }

    pub mod p2_p1 {
        use vp_p2_p1::{MERGE_PROOF, PROGRAM_1_PROOF, PROGRAM_2_PROOF};

        use super::*;

        lazy_static! {
            pub static ref VP_1_PROOF: ProofWithPublicInputs<F, C, D> = build_verified_program_leaf(
                &PROGRAM_1.circuit.verifier_only,
                &PROGRAM_1_PROOF,
                &P1_BUILT_EVENTS.proof,
            );
            pub static ref VP_2_PROOF: ProofWithPublicInputs<F, C, D> = build_verified_program_leaf(
                &PROGRAM_2.circuit.verifier_only,
                &PROGRAM_2_PROOF,
                &P2_BUILT_EVENTS.proof,
            );
            pub static ref VP_MERGE_PROOF: ProofWithPublicInputs<F, C, D> =
                build_verified_program_branch(&MERGE_PROOF, true, &VP_2_PROOF, true, &VP_1_PROOF);
        }
    }

    #[test]
    fn verify_simple() -> Result<()> {
        let leaf_1_proof = LEAF.prove(&BRANCH, &p1_p2::VP_MERGE_PROOF)?;
        LEAF.circuit.verify(leaf_1_proof.clone())?;

        let leaf_2_proof = LEAF.prove(&BRANCH, &p2_p1::VP_MERGE_PROOF)?;
        LEAF.circuit.verify(leaf_2_proof.clone())?;

        let branch_proof = BRANCH.prove(&MERGE_PROOF, true, &leaf_1_proof, true, &leaf_2_proof)?;
        BRANCH.circuit.verify(branch_proof.clone())?;

        Ok(())
    }
}

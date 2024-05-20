use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::{HashOutTarget, RichField};
use plonky2::iop::target::{BoolTarget, Target};
use plonky2::iop::witness::{PartialWitness, WitnessWrite};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::config::{AlgebraicHasher, GenericConfig};
use plonky2::plonk::proof::{ProofWithPublicInputs, ProofWithPublicInputsTarget};

use crate::circuits::verify_program;
use crate::connect_arrays;

pub struct ProgramSetVerifierTargets<const D: usize> {
    /// The program proof
    pub program_set_proof: ProofWithPublicInputsTarget<D>,

    /// The presence flag for the event root
    pub events_present: BoolTarget,

    /// The event root
    pub event_root: HashOutTarget,
}

pub struct ProgramVerifierSubCircuit<const D: usize> {
    pub targets: ProgramSetVerifierTargets<D>,
}

impl<const D: usize> ProgramSetVerifierTargets<D> {
    #[must_use]
    pub fn build_targets<F, C>(
        builder: &mut CircuitBuilder<F, D>,
        program: &verify_program::BranchCircuit<F, C, D>,
    ) -> Self
    where
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F>,
        <C as GenericConfig<D>>::Hasher: AlgebraicHasher<F>, {
        let circuit = &program.circuit;
        let program_set_proof = builder.add_virtual_proof_with_pis(&circuit.common);
        let verifier = builder.constant_verifier_data(&circuit.verifier_only);

        builder.verify_proof::<C>(&program_set_proof, &verifier, &circuit.common);

        let events_present = program
            .events
            .indices
            .hash_present
            .get_target(&program_set_proof.public_inputs);
        let event_root = program
            .events
            .indices
            .hash
            .get_target(&program_set_proof.public_inputs);

        let cast_root = program
            .cast_root
            .indices
            .values
            .get_target(&program_set_proof.public_inputs);
        let program_ids = program
            .program_id
            .indices
            .unpruned_hash
            .get_target(&program_set_proof.public_inputs);

        connect_arrays(builder, cast_root, program_ids.elements);

        Self {
            program_set_proof,
            events_present,
            event_root,
        }
    }
}

impl<const D: usize> ProgramSetVerifierTargets<D> {
    #[must_use]
    pub fn build(self, _public_inputs: &[Target]) -> ProgramVerifierSubCircuit<D> {
        ProgramVerifierSubCircuit { targets: self }
    }
}

impl<const D: usize> ProgramVerifierSubCircuit<D> {
    pub fn set_witness<F, C>(
        &self,
        inputs: &mut PartialWitness<F>,
        program_set_proof: &ProofWithPublicInputs<F, C, D>,
    ) where
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F>,
        <C as GenericConfig<D>>::Hasher: AlgebraicHasher<F>, {
        inputs.set_proof_with_pis_target(&self.targets.program_set_proof, program_set_proof);
    }
}

use itertools::chain;
use plonky2::field::extension::Extendable;
use plonky2::field::types::Field;
use plonky2::hash::hash_types::{HashOutTarget, RichField};
use plonky2::hash::poseidon2::Poseidon2Hash;
use plonky2::iop::target::{BoolTarget, Target};
use plonky2::iop::witness::{PartialWitness, Witness, WitnessWrite};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::{
    CircuitData, CommonCircuitData, VerifierCircuitTarget, VerifierOnlyCircuitData,
};
use plonky2::plonk::config::{AlgebraicHasher, GenericConfig};
use plonky2::plonk::proof::{ProofWithPublicInputs, ProofWithPublicInputsTarget};

use crate::circuits::build_event_root;
use crate::indices::{ArrayTargetIndex, BoolTargetIndex, HashOutTargetIndex, TargetIndex};
use crate::{dummy_circuit, select_verifier};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct ProgramPublicIndices {
    /// The indices of each of the elements of the program hash
    pub program_hash: ArrayTargetIndex<TargetIndex, 4>,

    /// The index of the presence flag for the event root
    pub events_present: BoolTargetIndex,

    /// The indices of each of the elements of event root
    pub event_root: HashOutTargetIndex,

    /// The indices of each of the elements of cast list
    pub call_list: ArrayTargetIndex<TargetIndex, 4>,

    /// The indices of each of the elements of cast root
    pub cast_root: HashOutTargetIndex,
}

pub struct ProgramVerifierTargets<const D: usize> {
    /// The program verifer
    pub program_verifier: VerifierCircuitTarget,

    /// The program verifier hash
    pub program_verifier_hash: HashOutTarget,

    /// The program proof
    pub program_proof: ProofWithPublicInputsTarget<D>,

    /// The program id
    pub program_id: [Target; 4],

    /// The presence flag for the event root
    pub events_present: BoolTarget,

    /// The event root
    pub event_root: HashOutTarget,

    /// The call list root
    pub call_list: [Target; 4],

    /// The cast list root
    pub cast_root: HashOutTarget,
}

pub struct ProgramVerifierSubCircuit<const D: usize> {
    pub targets: ProgramVerifierTargets<D>,
}

impl<const D: usize> ProgramVerifierTargets<D> {
    #[must_use]
    pub fn build_targets<F, C>(
        builder: &mut CircuitBuilder<F, D>,
        progam_circuit_indices: &ProgramPublicIndices,
        program_circuit_common: &CommonCircuitData<F, D>,
    ) -> Self
    where
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F>,
        <C as GenericConfig<D>>::Hasher: AlgebraicHasher<F>, {
        let program_proof = builder.add_virtual_proof_with_pis(program_circuit_common);
        let program_verifier =
            builder.add_virtual_verifier_data(program_circuit_common.config.fri_config.cap_height);
        builder.verify_proof::<C>(&program_proof, &program_verifier, program_circuit_common);

        let program_verifier_hash = builder.hash_n_to_hash_no_pad::<Poseidon2Hash>(
            chain!(
                [&program_verifier.circuit_digest],
                &program_verifier.constants_sigmas_cap.0,
            )
            .flat_map(|v| &v.elements)
            .copied()
            .collect(),
        );

        let program_id = progam_circuit_indices
            .program_hash
            .get_target(&program_proof.public_inputs);
        let events_present = progam_circuit_indices
            .events_present
            .get_target(&program_proof.public_inputs);
        let event_root = progam_circuit_indices
            .event_root
            .get_target(&program_proof.public_inputs);
        let call_list = progam_circuit_indices
            .call_list
            .get_target(&program_proof.public_inputs);
        let cast_root = progam_circuit_indices
            .cast_root
            .get_target(&program_proof.public_inputs);

        Self {
            program_verifier,
            program_verifier_hash,
            program_proof,
            program_id,
            events_present,
            event_root,
            call_list,
            cast_root,
        }
    }
}

impl<const D: usize> ProgramVerifierTargets<D> {
    #[must_use]
    pub fn build(self, _public_inputs: &[Target]) -> ProgramVerifierSubCircuit<D> {
        ProgramVerifierSubCircuit { targets: self }
    }
}

impl<const D: usize> ProgramVerifierSubCircuit<D> {
    pub fn set_witness<F, C>(
        &self,
        inputs: &mut PartialWitness<F>,
        program_verifier: &VerifierOnlyCircuitData<C, D>,
        program_proof: &ProofWithPublicInputs<F, C, D>,
    ) where
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F>,
        <C as GenericConfig<D>>::Hasher: AlgebraicHasher<F>, {
        inputs.set_verifier_data_target(&self.targets.program_verifier, program_verifier);
        inputs.set_proof_with_pis_target(&self.targets.program_proof, program_proof);
    }
}

pub struct EventRootVerifierTargets<C: GenericConfig<D>, const D: usize> {
    /// The event root proof
    pub event_root_proof: ProofWithPublicInputsTarget<D>,

    /// The event owner
    pub event_owner: [Target; 4],

    /// If events are present or not
    pub events_present: BoolTarget,

    /// The event root (rp_hash)
    pub event_root: HashOutTarget,

    /// The event root (vm hash)
    pub vm_event_root: HashOutTarget,

    /// The circuit for absent event proofs
    zero_circuit: CircuitData<C::F, C, D>,

    /// The event owner for absent event proofs
    zero_circuit_event_owner: [Target; 4],
}

pub struct EventRootVerifierSubCircuit<C: GenericConfig<D>, const D: usize> {
    pub targets: EventRootVerifierTargets<C, D>,
}

impl<C: GenericConfig<D>, const D: usize> EventRootVerifierTargets<C, D> {
    #[must_use]
    pub fn build_targets(
        builder: &mut CircuitBuilder<C::F, D>,
        event_root_circuit: &build_event_root::BranchCircuit<C::F, C, D>,
    ) -> Self
    where
        <C as GenericConfig<D>>::Hasher: AlgebraicHasher<C::F>, {
        let circuit = &event_root_circuit.circuit;

        let zero_circuit_event_owner = event_root_circuit.event_owner.targets.inputs.values;
        let zero_circuit = dummy_circuit::<C, D>(&circuit.common, |builder| {
            let hash = event_root_circuit
                .hash
                .targets
                .inputs
                .unpruned_hash
                .elements;
            let vm_hash = event_root_circuit
                .vm_hash
                .targets
                .inputs
                .unpruned_hash
                .elements;
            builder.register_public_inputs(&zero_circuit_event_owner);
            builder.register_public_inputs(&hash);
            builder.register_public_inputs(&vm_hash);
            for v in hash {
                builder.assert_zero(v);
            }
            for v in vm_hash {
                builder.assert_zero(v);
            }
        });
        let event_root_proof = builder.add_virtual_proof_with_pis(&circuit.common);
        let real_verifier = builder.constant_verifier_data(&circuit.verifier_only);
        let zero_verifier = builder.constant_verifier_data(&zero_circuit.verifier_only);
        let events_present = builder.add_virtual_bool_target_safe();

        let verifier = select_verifier(builder, events_present, &real_verifier, &zero_verifier);
        builder.verify_proof::<C>(&event_root_proof, &verifier, &circuit.common);

        let event_owner = event_root_circuit
            .event_owner
            .indices
            .values
            .get_target(&event_root_proof.public_inputs);
        let event_root = event_root_circuit
            .hash
            .indices
            .unpruned_hash
            .get_target(&event_root_proof.public_inputs);
        let vm_event_root = event_root_circuit
            .vm_hash
            .indices
            .unpruned_hash
            .get_target(&event_root_proof.public_inputs);

        Self {
            event_root_proof,
            events_present,
            event_owner,
            event_root,
            vm_event_root,
            zero_circuit,
            zero_circuit_event_owner,
        }
    }
}

impl<C: GenericConfig<D>, const D: usize> EventRootVerifierTargets<C, D> {
    #[must_use]
    pub fn build(self, _public_inputs: &[Target]) -> EventRootVerifierSubCircuit<C, D> {
        EventRootVerifierSubCircuit { targets: self }
    }
}

impl<C: GenericConfig<D>, const D: usize> EventRootVerifierSubCircuit<C, D> {
    pub fn set_witness(
        &self,
        inputs: &mut PartialWitness<C::F>,
        event_root_proof: Result<&ProofWithPublicInputs<C::F, C, D>, [C::F; 4]>,
    ) where
        <C as GenericConfig<D>>::Hasher: AlgebraicHasher<C::F>, {
        let storage;
        let event_root_proof = match event_root_proof {
            Ok(proof) => proof,
            Err(owner) => {
                let mut dummy_inputs = PartialWitness::new();
                dummy_inputs.set_target_arr(&self.targets.zero_circuit_event_owner, &owner);
                // Zero out all other inputs
                for i in 0..self.targets.zero_circuit.common.num_public_inputs {
                    let target = self.targets.zero_circuit.prover_only.public_inputs[i];
                    if dummy_inputs.try_get_target(target).is_none() {
                        dummy_inputs.set_target(target, <C::F>::ZERO);
                    }
                }
                storage = self.targets.zero_circuit.prove(dummy_inputs).unwrap();
                &storage
            }
        };

        inputs.set_proof_with_pis_target(&self.targets.event_root_proof, event_root_proof);
    }
}

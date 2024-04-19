//! Circuits for proving events correspond to a proof

use anyhow::Result;
use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::{HashOut, RichField, NUM_HASH_OUT_ELTS};
use plonky2::iop::witness::PartialWitness;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::{CircuitConfig, CircuitData};
use plonky2::plonk::config::{AlgebraicHasher, GenericConfig};
use plonky2::plonk::proof::{ProofWithPublicInputs, ProofWithPublicInputsTarget};

use super::{build_event_root, merge};
use crate::connect_arrays;
use crate::subcircuits::{propagate, unbounded, unpruned};

pub mod core;

pub struct LeafTargets<const D: usize> {
    pub event_proof: ProofWithPublicInputsTarget<D>,
}

pub struct LeafCircuit<F, C, const D: usize>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>, {
    /// The recursion subcircuit
    pub unbounded: unbounded::LeafSubCircuit,

    /// The program identifier
    pub program_id: unpruned::LeafSubCircuit,

    // The events list
    pub events: merge::embed::LeafSubCircuit,

    /// The cast list
    pub cast_list: propagate::LeafSubCircuit<NUM_HASH_OUT_ELTS>,

    /// The program verifier
    pub program_verifier: core::ProgramVerifierSubCircuit<D>,

    /// The event root verifier
    pub event_verifier: core::EventRootVerifierSubCircuit<D>,

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
        program: &impl core::Circuit<F, C, D>,
        event_root: &build_event_root::BranchCircuit<F, C, D>,
    ) -> Self {
        let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());

        let unbounded_inputs = unbounded::SubCircuitInputs::default(&mut builder);
        let program_id_inputs = unpruned::SubCircuitInputs::default(&mut builder);
        let events_inputs = merge::embed::SubCircuitInputs::default(&mut builder);
        let cast_list_inputs = propagate::SubCircuitInputs::default(&mut builder);

        let unbounded_targets = unbounded_inputs.build_leaf::<F, C, D>(&mut builder);
        let program_id_targets = program_id_inputs.build_leaf::<F, D>(&mut builder);
        let events_targets = events_inputs.build_leaf::<F, D>(&mut builder);
        let cast_list_targets = cast_list_inputs.build_leaf::<F, D>(&mut builder);

        let program_verifier_targets =
            core::ProgramVerifierTargets::build_targets(&mut builder, program);
        let event_verifier_targets =
            core::EventRootVerifierTargets::build_targets(&mut builder, event_root);

        // Connect the proofs
        connect_arrays(
            &mut builder,
            program_verifier_targets.program_hash,
            program_id_targets.inputs.unpruned_hash.elements,
        );
        connect_arrays(
            &mut builder,
            event_verifier_targets.event_owner,
            program_id_targets.inputs.unpruned_hash.elements,
        );
        builder.connect_hashes(
            program_verifier_targets.event_root,
            event_verifier_targets.vm_event_root,
        );
        builder.connect_hashes(
            events_targets.inputs.hash,
            event_verifier_targets.event_root,
        );
        connect_arrays(
            &mut builder,
            program_verifier_targets.cast_root.elements,
            cast_list_targets.inputs.values,
        );

        let circuit = builder.build();

        let public_inputs = &circuit.prover_only.public_inputs;
        let unbounded = unbounded_targets.build(public_inputs);
        let program_id = program_id_targets.build(public_inputs);
        let events = events_targets.build(public_inputs);
        let cast_list = cast_list_targets.build(public_inputs);
        let program_verifier = program_verifier_targets.build(public_inputs);
        let event_verifier = event_verifier_targets.build(public_inputs);

        Self {
            unbounded,
            program_id,
            events,
            cast_list,
            program_verifier,
            event_verifier,
            circuit,
        }
    }

    pub fn prove(
        &self,
        branch: &BranchCircuit<F, C, D>,
        program_proof: &ProofWithPublicInputs<F, C, D>,
        event_root_proof: &ProofWithPublicInputs<F, C, D>,
    ) -> Result<ProofWithPublicInputs<F, C, D>> {
        let mut inputs = PartialWitness::new();
        self.unbounded.set_witness(&mut inputs, &branch.circuit);
        self.program_verifier
            .set_witness(&mut inputs, program_proof);
        self.event_verifier
            .set_witness(&mut inputs, event_root_proof);
        self.circuit.prove(inputs)
    }

    pub fn prove_unsafe(
        &self,
        branch: &BranchCircuit<F, C, D>,
        program_proof: &ProofWithPublicInputs<F, C, D>,
        program_id: HashOut<F>,
        event_root: HashOut<F>,
        cast_root: HashOut<F>,
    ) -> Result<ProofWithPublicInputs<F, C, D>> {
        let mut inputs = PartialWitness::new();
        self.unbounded.set_witness(&mut inputs, &branch.circuit);
        self.program_verifier
            .set_witness(&mut inputs, program_proof);
        self.program_id.set_witness(&mut inputs, program_id);
        self.events.set_witness(&mut inputs, Some(event_root));
        self.cast_list.set_witness(&mut inputs, cast_root.elements);
        self.circuit.prove(inputs)
    }
}

pub struct BranchCircuit<F, C, const D: usize>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>, {
    /// The recursion subcircuit
    pub unbounded: unbounded::BranchSubCircuit<D>,

    /// The program identifier
    pub program_id: unpruned::BranchSubCircuit,

    // The events list
    pub events: merge::embed::BranchSubCircuit<D>,

    /// The cast list
    pub cast_list: propagate::BranchSubCircuit<NUM_HASH_OUT_ELTS>,

    pub circuit: CircuitData<F, C, D>,
}

impl<F, C, const D: usize> BranchCircuit<F, C, D>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    <C as GenericConfig<D>>::Hasher: AlgebraicHasher<F>,
{
    #[must_use]
    pub fn new(circuit_config: &CircuitConfig) -> Self {
        let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());

        let _unbounded_inputs = unbounded::SubCircuitInputs::default(&mut builder);

        todo!()
    }
}

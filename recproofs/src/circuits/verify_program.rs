//! Circuits for proving events correspond to a proof

use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::{HashOutTarget, RichField, NUM_HASH_OUT_ELTS};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::{CircuitConfig, CircuitData};
use plonky2::plonk::config::{AlgebraicHasher, GenericConfig};
use plonky2::plonk::proof::ProofWithPublicInputsTarget;

use super::build_event_root;
use crate::subcircuits::{propagate, unbounded, unpruned};

pub mod core;

pub struct LeafTargets<const D: usize> {
    /// The proof of event accumulation
    pub event_root: ProofWithPublicInputsTarget<D>,
}

pub struct LeafCircuit<F, C, const D: usize>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>, {
    /// The recursion subcircuit
    pub unbounded: unbounded::LeafSubCircuit,

    /// The program verifier
    pub program_verifier: core::LeafSubCircuit<D>,

    /// The program identifier
    pub program_id: unpruned::LeafSubCircuit,

    /// The cast list
    pub cast_list: propagate::LeafSubCircuit<NUM_HASH_OUT_ELTS>,

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
        _event_root: build_event_root::BranchCircuit<F, C, D>,
    ) -> Self {
        let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());

        let unbounded_inputs = unbounded::SubCircuitInputs::default(&mut builder);
        let program_verifier_inputs = core::SubCircuitInputs::default(&mut builder);
        let program_id_inputs = unpruned::SubCircuitInputs::default(&mut builder);
        let cast_list_inputs = propagate::SubCircuitInputs::default(&mut builder);

        let unbounded_targets = unbounded_inputs.build_leaf::<F, C, D>(&mut builder);
        let program_verifier_targets =
            program_verifier_inputs.build_leaf::<F, C, D>(&mut builder, program);
        let program_id_targets = program_id_inputs.build_leaf::<F, D>(&mut builder);
        let cast_list_targets = cast_list_inputs.build_leaf::<F, D>(&mut builder);

        builder.connect_hashes(
            HashOutTarget {
                elements: program_verifier_targets.program_hash,
            },
            program_id_targets.inputs.unpruned_hash,
        );

        let circuit = builder.build();

        let public_inputs = &circuit.prover_only.public_inputs;
        let unbounded = unbounded_targets.build(public_inputs);
        let program_verifier = program_verifier_targets.build(public_inputs);
        let program_id = program_id_targets.build(public_inputs);
        let cast_list = cast_list_targets.build(public_inputs);

        Self {
            unbounded,
            program_verifier,
            program_id,
            cast_list,
            circuit,
        }
    }
}

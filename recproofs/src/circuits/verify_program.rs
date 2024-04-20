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
use crate::subcircuits::unpruned::PartialAllowed;
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
        program: &dyn core::Circuit<F, C, D>,
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
        builder.connect(
            events_targets.inputs.hash_present.target,
            program_verifier_targets.events_present.target,
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
    pub program_id: unpruned::BranchSubCircuit<PartialAllowed>,

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
    pub fn new(
        circuit_config: &CircuitConfig,
        mc: &merge::BranchCircuit<F, C, D>,
        leaf: &LeafCircuit<F, C, D>,
    ) -> Self {
        let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());

        let unbounded_inputs = unbounded::SubCircuitInputs::default(&mut builder);
        let program_id_inputs = unpruned::SubCircuitInputs::default(&mut builder);
        let events_inputs = merge::embed::SubCircuitInputs::default(&mut builder);
        let cast_list_inputs = propagate::SubCircuitInputs::default(&mut builder);

        let unbounded_targets =
            unbounded_inputs.build_branch(&mut builder, &leaf.unbounded, &leaf.circuit);
        let program_id_targets = program_id_inputs.build_extended_branch(
            &mut builder,
            &leaf.program_id.indices,
            &unbounded_targets.left_proof,
            &unbounded_targets.right_proof,
            true,
        );
        let events_targets = events_inputs.build_branch(
            &mut builder,
            mc,
            &leaf.events.indices,
            &unbounded_targets.left_proof,
            &unbounded_targets.right_proof,
        );
        let cast_list_targets = cast_list_inputs.build_branch(
            &mut builder,
            &leaf.cast_list.indices,
            &unbounded_targets.left_proof,
            &unbounded_targets.right_proof,
        );

        let circuit = builder.build();

        let public_inputs = &circuit.prover_only.public_inputs;
        let unbounded = unbounded_targets.build(&leaf.unbounded, public_inputs);
        let program_id = program_id_targets.build(&leaf.program_id.indices, public_inputs);
        let events = events_targets.build(&leaf.events.indices, public_inputs);
        let cast_list = cast_list_targets.build(&leaf.cast_list.indices, public_inputs);

        Self {
            unbounded,
            program_id,
            events,
            cast_list,
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
        right_proof: Option<(bool, &ProofWithPublicInputs<F, C, D>)>,
    ) -> Result<ProofWithPublicInputs<F, C, D>> {
        let mut inputs = PartialWitness::new();
        let partial = right_proof.is_none();
        let (right_is_leaf, right_proof) = if let Some(right_proof) = right_proof {
            right_proof
        } else {
            (left_is_leaf, left_proof)
        };
        self.unbounded.set_witness(
            &mut inputs,
            left_is_leaf,
            left_proof,
            right_is_leaf,
            right_proof,
        );
        self.program_id.set_witness(&mut inputs, None, partial);
        self.events.set_witness(&mut inputs, merge);
        self.circuit.prove(inputs)
    }
}

#[cfg(test)]
mod test {
    use lazy_static::lazy_static;
    use plonky2::field::types::Field;
    use plonky2::hash::hash_types::HashOutTarget;
    use plonky2::iop::target::{BoolTarget, Target};
    use plonky2::iop::witness::WitnessWrite;

    use self::core::{Circuit, CircuitPublicIndices};
    use super::*;
    use crate::test_utils::{fast_test_circuit_config, hash_branch, hash_branch_bytes, C, D, F};
    use crate::{find_bool, find_hash, find_targets, Event, EventType};

    const CONFIG: CircuitConfig = fast_test_circuit_config();

    struct DummyCircuit<F, C, const D: usize>
    where
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F>, {
        /// The program hash
        pub program_hash: [Target; 4],

        /// The presence flag for the event root
        pub events_present: BoolTarget,

        /// The event root
        pub event_root: HashOutTarget,

        /// The cast list root
        pub cast_root: HashOutTarget,

        pub circuit: CircuitData<F, C, D>,

        pub indices: CircuitPublicIndices,
    }

    impl<F, C, const D: usize> Circuit<F, C, D> for DummyCircuit<F, C, D>
    where
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F>,
    {
        fn get_circuit_data(&self) -> &CircuitData<F, C, D> { &self.circuit }

        fn get_indices(&self) -> CircuitPublicIndices { self.indices }
    }

    impl<F, C, const D: usize> DummyCircuit<F, C, D>
    where
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F>,
    {
        #[must_use]
        pub fn new(circuit_config: &CircuitConfig) -> Self {
            let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());
            let program_hash = builder.add_virtual_target_arr();
            let events_present = builder.add_virtual_bool_target_safe();
            let event_root = builder.add_virtual_hash();
            let cast_root = builder.add_virtual_hash();
            builder.register_public_inputs(&program_hash);
            builder.register_public_input(events_present.target);
            builder.register_public_inputs(&event_root.elements);
            builder.register_public_inputs(&cast_root.elements);

            let circuit = builder.build();

            let public_inputs = &circuit.prover_only.public_inputs;
            let indices = CircuitPublicIndices {
                program_hash: find_targets(public_inputs, program_hash),
                events_present: find_bool(public_inputs, events_present),
                event_root: find_hash(public_inputs, event_root),
                cast_root: find_hash(public_inputs, cast_root),
            };

            Self {
                program_hash,
                events_present,
                event_root,
                cast_root,
                circuit,
                indices,
            }
        }

        pub fn prove(
            &self,
            program_hash: [F; 4],
            event_root: Option<HashOut<F>>,
            cast_root: HashOut<F>,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            inputs.set_target_arr(&self.program_hash, &program_hash);
            inputs.set_bool_target(self.events_present, event_root.is_some());
            inputs.set_hash_target(self.event_root, event_root.unwrap_or_default());
            inputs.set_hash_target(self.cast_root, cast_root);
            self.circuit.prove(inputs)
        }
    }

    lazy_static! {
        static ref EVENT_LEAF: build_event_root::LeafCircuit<F, C, D> =
            build_event_root::LeafCircuit::new(&CONFIG);
        static ref EVENT_BRANCH: build_event_root::BranchCircuit<F, C, D> =
            build_event_root::BranchCircuit::new(&CONFIG, &EVENT_LEAF);
        static ref MERGE_LEAF: merge::LeafCircuit<F, C, D> = merge::LeafCircuit::new(&CONFIG);
        static ref MERGE_BRANCH: merge::BranchCircuit<F, C, D> =
            merge::BranchCircuit::new(&CONFIG, &MERGE_LEAF);
        static ref PROGRAM: DummyCircuit<F, C, D> = DummyCircuit::new(&CONFIG);
        static ref LEAF: LeafCircuit<F, C, D> = LeafCircuit::new(&CONFIG, &*PROGRAM, &EVENT_BRANCH);
        static ref BRANCH: BranchCircuit<F, C, D> =
            BranchCircuit::new(&CONFIG, &MERGE_BRANCH, &LEAF);
    }

    fn build_event(e: Event<F>) -> Result<ProofWithPublicInputs<F, C, D>> {
        let proof = EVENT_LEAF.prove(e, Some(e.hash()), Some(e.byte_wise_hash()), &EVENT_BRANCH)?;
        EVENT_LEAF.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    pub struct BuiltEvent {
        pub proof: ProofWithPublicInputs<F, C, D>,
        pub hash: HashOut<F>,
        pub vm_hash: HashOut<F>,
    }

    fn build_events(l: Event<F>, r: Event<F>) -> Result<BuiltEvent> {
        let l_proof = build_event(l)?;
        let r_proof = build_event(r)?;
        let branch_hash = hash_branch(&l.hash(), &r.hash());
        let branch_bytes_hash = hash_branch_bytes(&l.byte_wise_hash(), &r.byte_wise_hash());

        let branch_proof = EVENT_BRANCH.prove(
            Some(branch_hash),
            Some(branch_bytes_hash),
            Some(l.owner),
            true,
            &l_proof,
            Some((true, &r_proof)),
        )?;
        EVENT_BRANCH.circuit.verify(branch_proof.clone())?;

        Ok(BuiltEvent {
            proof: branch_proof,
            hash: branch_hash,
            vm_hash: branch_bytes_hash,
        })
    }

    fn make_program(
        program_hash: [F; 4],
        event_root: Option<HashOut<F>>,
        cast_root: HashOut<F>,
    ) -> Result<ProofWithPublicInputs<F, C, D>> {
        let program_proof = PROGRAM.prove(program_hash, event_root, cast_root)?;
        PROGRAM.circuit.verify(program_proof.clone())?;
        Ok(program_proof)
    }

    fn merge_events(
        a: Event<F>,
        b: Event<F>,
    ) -> Result<(ProofWithPublicInputs<F, C, D>, HashOut<F>)> {
        let a = a.hash();
        let b = b.hash();
        let merged_hash = hash_branch(&a, &b);
        let merge_proof = MERGE_LEAF.prove(&MERGE_BRANCH, Some(a), Some(b), Some(merged_hash))?;
        MERGE_LEAF.circuit.verify(merge_proof.clone())?;
        Ok((merge_proof, merged_hash))
    }

    fn merge_merges(
        l_leaf: bool,
        l: &ProofWithPublicInputs<F, C, D>,
        r_leaf: bool,
        r: &ProofWithPublicInputs<F, C, D>,
    ) -> Result<ProofWithPublicInputs<F, C, D>> {
        let merge_proof = MERGE_BRANCH.prove(l_leaf, l, r_leaf, r)?;
        MERGE_BRANCH.circuit.verify(merge_proof.clone())?;
        Ok(merge_proof)
    }

    #[test]
    fn verify_leaf() -> Result<()> {
        let program_hash_1 = [4, 8, 15, 16].map(F::from_canonical_u64);
        let program_hash_2 = [2, 3, 4, 2].map(F::from_canonical_u64);

        let zero_val = [F::ZERO; 4];
        let non_zero_val_1 = [3, 1, 4, 15].map(F::from_canonical_u64);
        let non_zero_val_2 = [1, 6, 180, 33].map(F::from_canonical_u64);

        // Duplicate or conflicting events are actually fine as far as this circuit
        // cares
        let p1_events = [
            Event {
                address: 42,
                owner: program_hash_1,
                ty: EventType::Read,
                value: zero_val,
            },
            Event {
                address: 84,
                owner: program_hash_1,
                ty: EventType::Write,
                value: non_zero_val_1,
            },
        ];
        let p1_built_events = build_events(p1_events[0], p1_events[1])?;
        let p2_events = [
            Event {
                address: 42,
                owner: program_hash_2,
                ty: EventType::Write,
                value: non_zero_val_2,
            },
            Event {
                address: 84,
                owner: program_hash_2,
                ty: EventType::Ensure,
                value: non_zero_val_1,
            },
        ];
        let p2_built_events = build_events(p2_events[0], p2_events[1])?;
        let cast_root = hash_branch_bytes(&program_hash_1.into(), &program_hash_2.into());

        let program_1_proof =
            make_program(program_hash_1, Some(p1_built_events.vm_hash), cast_root)?;
        let program_2_proof =
            make_program(program_hash_2, Some(p2_built_events.vm_hash), cast_root)?;

        let (merge_42, _hash_42) = merge_events(p1_events[0], p2_events[0])?;
        let (merge_80, _hash_80) = merge_events(p1_events[1], p2_events[1])?;

        let merge_proof = merge_merges(true, &merge_42, true, &merge_80)?;

        let leaf_1_proof = LEAF.prove(&BRANCH, &program_1_proof, &p1_built_events.proof)?;
        LEAF.circuit.verify(leaf_1_proof.clone())?;

        let leaf_2_proof = LEAF.prove(&BRANCH, &program_2_proof, &p2_built_events.proof)?;
        LEAF.circuit.verify(leaf_2_proof.clone())?;

        let branch_proof = BRANCH.prove(
            &merge_proof,
            true,
            &leaf_1_proof,
            Some((true, &leaf_2_proof)),
        )?;
        BRANCH.circuit.verify(branch_proof.clone())?;

        Ok(())
    }
}

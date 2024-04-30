//! Circuits for proving events correspond to a program proof

use anyhow::Result;
use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::{HashOut, RichField, NUM_HASH_OUT_ELTS};
use plonky2::iop::witness::PartialWitness;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::{
    CircuitConfig, CircuitData, CommonCircuitData, VerifierOnlyCircuitData,
};
use plonky2::plonk::config::{AlgebraicHasher, GenericConfig};
use plonky2::plonk::proof::ProofWithPublicInputs;

use super::{build_event_root, merge};
use crate::connect_arrays;
use crate::subcircuits::{propagate, unbounded, unpruned};

pub mod core;

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

    /// The call list
    pub call_list: propagate::LeafSubCircuit<NUM_HASH_OUT_ELTS>,

    /// The cast list root
    pub cast_root: propagate::LeafSubCircuit<NUM_HASH_OUT_ELTS>,

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
        program_circuit_indices: &core::ProgramPublicIndices,
        program_circuit_common: &CommonCircuitData<F, D>,
        event_root: &build_event_root::BranchCircuit<F, C, D>,
    ) -> Self {
        let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());

        let unbounded_inputs = unbounded::SubCircuitInputs::default(&mut builder);
        let program_id_inputs = unpruned::SubCircuitInputs::default(&mut builder);
        let events_inputs = merge::embed::SubCircuitInputs::default(&mut builder);
        let call_list_inputs = propagate::SubCircuitInputs::default(&mut builder);
        let cast_root_inputs = propagate::SubCircuitInputs::default(&mut builder);

        let unbounded_targets = unbounded_inputs.build_leaf::<F, C, D>(&mut builder);
        let program_id_targets = program_id_inputs.build_leaf::<F, D>(&mut builder);
        let events_targets = events_inputs.build_leaf::<F, D>(&mut builder);
        let call_list_targets = call_list_inputs.build_leaf::<F, D>(&mut builder);
        let cast_root_targets = cast_root_inputs.build_leaf::<F, D>(&mut builder);

        let program_verifier_targets = core::ProgramVerifierTargets::build_targets::<F, C>(
            &mut builder,
            program_circuit_indices,
            program_circuit_common,
        );
        let event_verifier_targets =
            core::EventRootVerifierTargets::build_targets(&mut builder, event_root);

        // Connect the proofs
        connect_arrays(
            &mut builder,
            program_verifier_targets.program_id,
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
            program_verifier_targets.call_list,
            call_list_targets.inputs.values,
        );
        connect_arrays(
            &mut builder,
            program_verifier_targets.cast_root.elements,
            cast_root_targets.inputs.values,
        );

        let circuit = builder.build();

        let public_inputs = &circuit.prover_only.public_inputs;
        let unbounded = unbounded_targets.build(public_inputs);
        let program_id = program_id_targets.build(public_inputs);
        let events = events_targets.build(public_inputs);
        let call_list = call_list_targets.build(public_inputs);
        let cast_root = cast_root_targets.build(public_inputs);
        let program_verifier = program_verifier_targets.build(public_inputs);
        let event_verifier = event_verifier_targets.build(public_inputs);

        Self {
            unbounded,
            program_id,
            events,
            call_list,
            cast_root,
            program_verifier,
            event_verifier,
            circuit,
        }
    }

    pub fn prove(
        &self,
        branch: &BranchCircuit<F, C, D>,
        program_verifier: &VerifierOnlyCircuitData<C, D>,
        program_proof: &ProofWithPublicInputs<F, C, D>,
        event_root_proof: &ProofWithPublicInputs<F, C, D>,
    ) -> Result<ProofWithPublicInputs<F, C, D>> {
        let mut inputs = PartialWitness::new();
        self.unbounded.set_witness(&mut inputs, &branch.circuit);
        self.program_verifier
            .set_witness(&mut inputs, program_verifier, program_proof);
        self.event_verifier
            .set_witness(&mut inputs, event_root_proof);
        self.circuit.prove(inputs)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn prove_unsafe(
        &self,
        branch: &BranchCircuit<F, C, D>,
        program_verifier: &VerifierOnlyCircuitData<C, D>,
        program_proof: &ProofWithPublicInputs<F, C, D>,
        program_id: HashOut<F>,
        event_root: HashOut<F>,
        call_list: [F; 4],
        cast_root: HashOut<F>,
    ) -> Result<ProofWithPublicInputs<F, C, D>> {
        let mut inputs = PartialWitness::new();
        self.unbounded.set_witness(&mut inputs, &branch.circuit);
        self.program_verifier
            .set_witness(&mut inputs, program_verifier, program_proof);
        self.program_id.set_witness(&mut inputs, program_id);
        self.events.set_witness(&mut inputs, Some(event_root));
        self.call_list.set_witness(&mut inputs, call_list);
        self.cast_root.set_witness(&mut inputs, cast_root.elements);
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

    /// The call list
    pub call_list: propagate::BranchSubCircuit<NUM_HASH_OUT_ELTS>,

    /// The cast list root
    pub cast_root: propagate::BranchSubCircuit<NUM_HASH_OUT_ELTS>,

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
        let call_list_inputs = propagate::SubCircuitInputs::default(&mut builder);
        let cast_root_inputs = propagate::SubCircuitInputs::default(&mut builder);

        let unbounded_targets =
            unbounded_inputs.build_branch(&mut builder, &leaf.unbounded, &leaf.circuit);
        let program_id_targets = program_id_inputs.build_branch(
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
        let call_list_targets = call_list_inputs.build_branch(
            &mut builder,
            &leaf.call_list.indices,
            &unbounded_targets.left_proof,
            &unbounded_targets.right_proof,
        );
        let cast_root_targets = cast_root_inputs.build_branch(
            &mut builder,
            &leaf.cast_root.indices,
            &unbounded_targets.left_proof,
            &unbounded_targets.right_proof,
        );

        let circuit = builder.build();

        let public_inputs = &circuit.prover_only.public_inputs;
        let unbounded = unbounded_targets.build(&leaf.unbounded, public_inputs);
        let program_id = program_id_targets.build(&leaf.program_id.indices, public_inputs);
        let events = events_targets.build(&leaf.events.indices, public_inputs);
        let call_list = call_list_targets.build(&leaf.call_list.indices, public_inputs);
        let cast_root = cast_root_targets.build(&leaf.cast_root.indices, public_inputs);

        Self {
            unbounded,
            program_id,
            events,
            call_list,
            cast_root,
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
    use std::panic::catch_unwind;

    use lazy_static::lazy_static;
    use plonky2::field::types::Field;
    use plonky2::hash::hash_types::HashOutTarget;
    use plonky2::iop::target::{BoolTarget, Target};
    use plonky2::iop::witness::WitnessWrite;

    use self::core::ProgramPublicIndices;
    use super::*;
    use crate::circuits::build_event_root::test::{BRANCH as EVENT_BRANCH, LEAF as EVENT_LEAF};
    use crate::circuits::merge::test::{BRANCH as MERGE_BRANCH, LEAF as MERGE_LEAF};
    use crate::test_utils::{hash_branch, hash_branch_bytes, make_fs, C, CONFIG, D, F};
    use crate::{find_bool, find_hash, find_targets, Event, EventType};

    pub struct DummyCircuit<F, C, const D: usize>
    where
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F>, {
        /// The program hash
        pub program_hash: [Target; 4],

        /// The presence flag for the event root
        pub events_present: BoolTarget,

        /// The event root
        pub event_root: HashOutTarget,

        /// The call list
        pub call_list: [Target; 4],

        /// The cast list root
        pub cast_root: HashOutTarget,

        pub circuit: CircuitData<F, C, D>,
    }

    impl<F, C, const D: usize> DummyCircuit<F, C, D>
    where
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F>,
    {
        #[must_use]
        pub fn new(circuit_config: &CircuitConfig, dummy: bool) -> Self {
            let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());
            let program_hash = builder.add_virtual_target_arr();
            let events_present = builder.add_virtual_bool_target_safe();
            let event_root = builder.add_virtual_hash();
            let call_list = builder.add_virtual_target_arr();
            let cast_root = builder.add_virtual_hash();
            builder.register_public_inputs(&program_hash);
            builder.register_public_input(events_present.target);
            builder.register_public_inputs(&event_root.elements);
            builder.register_public_inputs(&call_list);
            builder.register_public_inputs(&cast_root.elements);

            // Make a dummy to change the circuit
            if dummy {
                let dummy = builder.add_virtual_target();
                let one = builder.one();
                builder.connect(dummy, one);
            }

            let circuit = builder.build();

            Self {
                program_hash,
                events_present,
                event_root,
                call_list,
                cast_root,
                circuit,
            }
        }

        pub fn get_indices(&self) -> ProgramPublicIndices {
            let public_inputs = &self.circuit.prover_only.public_inputs;
            ProgramPublicIndices {
                program_hash: find_targets(public_inputs, self.program_hash),
                events_present: find_bool(public_inputs, self.events_present),
                event_root: find_hash(public_inputs, self.event_root),
                call_list: find_targets(public_inputs, self.call_list),
                cast_root: find_hash(public_inputs, self.cast_root),
            }
        }

        pub fn prove(
            &self,
            program_hash: [F; 4],
            event_root: Option<HashOut<F>>,
            call_list: [F; 4],
            cast_root: HashOut<F>,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            inputs.set_target_arr(&self.program_hash, &program_hash);
            inputs.set_bool_target(self.events_present, event_root.is_some());
            inputs.set_hash_target(self.event_root, event_root.unwrap_or_default());
            inputs.set_target_arr(&self.call_list, &call_list);
            inputs.set_hash_target(self.cast_root, cast_root);
            self.circuit.prove(inputs)
        }
    }

    lazy_static! {
        pub static ref PROGRAM_1: DummyCircuit<F, C, D> = DummyCircuit::new(&CONFIG, false);
        pub static ref PROGRAM_1_INDICES: ProgramPublicIndices = PROGRAM_1.get_indices();
        pub static ref PROGRAM_2: DummyCircuit<F, C, D> = DummyCircuit::new(&CONFIG, true);
        pub static ref PROGRAM_2_INDICES: ProgramPublicIndices = PROGRAM_2.get_indices();
        pub static ref LEAF: LeafCircuit<F, C, D> = LeafCircuit::new(
            &CONFIG,
            &PROGRAM_1_INDICES,
            &PROGRAM_1.circuit.common,
            &EVENT_BRANCH
        );
        pub static ref BRANCH: BranchCircuit<F, C, D> =
            BranchCircuit::new(&CONFIG, &MERGE_BRANCH, &LEAF);
    }

    fn build_event(e: Event<F>) -> ProofWithPublicInputs<F, C, D> {
        let proof = EVENT_LEAF
            .prove(e, Some(e.hash()), Some(e.byte_wise_hash()), &EVENT_BRANCH)
            .unwrap();
        EVENT_LEAF.circuit.verify(proof.clone()).unwrap();
        proof
    }

    pub struct BuiltEvent {
        pub proof: ProofWithPublicInputs<F, C, D>,
        #[allow(dead_code)]
        pub hash: HashOut<F>,
        pub vm_hash: HashOut<F>,
    }

    pub fn build_events(l: Event<F>, r: Event<F>) -> BuiltEvent {
        let l_proof = build_event(l);
        let r_proof = build_event(r);
        let branch_hash = hash_branch(&l.hash(), &r.hash());
        let branch_bytes_hash = hash_branch_bytes(&l.byte_wise_hash(), &r.byte_wise_hash());

        let branch_proof = EVENT_BRANCH
            .prove(
                Some(branch_hash),
                Some(branch_bytes_hash),
                Some(l.owner),
                true,
                &l_proof,
                Some((true, &r_proof)),
            )
            .unwrap();
        EVENT_BRANCH.circuit.verify(branch_proof.clone()).unwrap();

        BuiltEvent {
            proof: branch_proof,
            hash: branch_hash,
            vm_hash: branch_bytes_hash,
        }
    }

    pub fn make_program(
        program: &DummyCircuit<F, C, D>,
        program_hash: [F; 4],
        event_root: Option<HashOut<F>>,
        call_list: [F; 4],
        cast_root: HashOut<F>,
    ) -> ProofWithPublicInputs<F, C, D> {
        let program_proof = program
            .prove(program_hash, event_root, call_list, cast_root)
            .unwrap();
        program.circuit.verify(program_proof.clone()).unwrap();
        program_proof
    }

    pub fn merge_hashes(
        a: Option<HashOut<F>>,
        b: Option<HashOut<F>>,
    ) -> ProofWithPublicInputs<F, C, D> {
        let merged_hash = match (a, b) {
            (None, None) => None,
            (None, Some(b)) => Some(b),
            (Some(a), None) => Some(a),
            (Some(a), Some(b)) => Some(hash_branch(&a, &b)),
        };
        let merge_proof = MERGE_LEAF.prove(&MERGE_BRANCH, a, b, merged_hash).unwrap();
        MERGE_LEAF.circuit.verify(merge_proof.clone()).unwrap();
        merge_proof
    }

    pub fn merge_events(a: Event<F>, b: Event<F>) -> ProofWithPublicInputs<F, C, D> {
        merge_hashes(Some(a.hash()), Some(b.hash()))
    }

    pub fn merge_merges(
        l_leaf: bool,
        l: &ProofWithPublicInputs<F, C, D>,
        r_leaf: bool,
        r: &ProofWithPublicInputs<F, C, D>,
    ) -> ProofWithPublicInputs<F, C, D> {
        let merge_proof = MERGE_BRANCH.prove(l_leaf, l, r_leaf, r).unwrap();
        MERGE_BRANCH.circuit.verify(merge_proof.clone()).unwrap();
        merge_proof
    }

    const ZERO_VAL: [F; 4] = [F::ZERO; 4];
    const PROGRAM_1_HASH: [F; 4] = make_fs([4, 8, 15, 16]);
    const PROGRAM_2_HASH: [F; 4] = make_fs([2, 3, 4, 2]);
    const NON_ZERO_VAL_1: [F; 4] = make_fs([3, 1, 4, 15]);
    const NON_ZERO_VAL_2: [F; 4] = make_fs([1, 6, 180, 33]);
    const CALL_LIST_1: [F; 4] = make_fs([86, 7, 5, 309]);
    const CALL_LIST_2: [F; 4] = make_fs([8, 67, 530, 9]);

    // Duplicate or conflicting events are actually fine as far as this circuit
    // cares
    const P1_EVENTS: [Event<F>; 2] = [
        Event {
            address: 42,
            owner: PROGRAM_1_HASH,
            ty: EventType::Read,
            value: ZERO_VAL,
        },
        Event {
            address: 84,
            owner: PROGRAM_1_HASH,
            ty: EventType::Write,
            value: ZERO_VAL,
        },
    ];
    const P2_EVENTS: [Event<F>; 2] = [
        Event {
            address: 42,
            owner: PROGRAM_2_HASH,
            ty: EventType::Write,
            value: NON_ZERO_VAL_2,
        },
        Event {
            address: 84,
            owner: PROGRAM_2_HASH,
            ty: EventType::Ensure,
            value: NON_ZERO_VAL_1,
        },
    ];

    lazy_static! {
        pub static ref P1_BUILT_EVENTS: BuiltEvent = build_events(P1_EVENTS[0], P1_EVENTS[1]);
        pub static ref P1_EVENTS_HASH: HashOut<F> =
            hash_branch(&P1_EVENTS[0].hash(), &P1_EVENTS[1].hash());
        pub static ref P2_BUILT_EVENTS: BuiltEvent = build_events(P2_EVENTS[0], P2_EVENTS[1]);
        pub static ref P2_EVENTS_HASH: HashOut<F> =
            hash_branch(&P2_EVENTS[0].hash(), &P2_EVENTS[1].hash());
    }

    /// Helpers with P1 to the left of P2
    pub mod p1_p2 {
        use super::*;

        lazy_static! {
            pub static ref CAST_ROOT: HashOut<F> =
                hash_branch_bytes(&PROGRAM_1_HASH.into(), &PROGRAM_2_HASH.into());
        }

        lazy_static! {
            pub static ref PROGRAM_1_PROOF: ProofWithPublicInputs<F, C, D> = make_program(
                &PROGRAM_1,
                PROGRAM_1_HASH,
                Some(P1_BUILT_EVENTS.vm_hash),
                CALL_LIST_1,
                *CAST_ROOT
            );
            pub static ref PROGRAM_2_PROOF: ProofWithPublicInputs<F, C, D> = make_program(
                &PROGRAM_2,
                PROGRAM_2_HASH,
                Some(P2_BUILT_EVENTS.vm_hash),
                CALL_LIST_1,
                *CAST_ROOT
            );
            pub static ref PROGRAM_2B_PROOF: ProofWithPublicInputs<F, C, D> = make_program(
                &PROGRAM_2,
                PROGRAM_2_HASH,
                Some(P2_BUILT_EVENTS.vm_hash),
                CALL_LIST_2,
                *CAST_ROOT
            );
        }

        lazy_static! {
            pub static ref MERGE_42_HASH: HashOut<F> =
                hash_branch(&P1_EVENTS[0].hash(), &P2_EVENTS[0].hash());
            pub static ref MERGE_80_HASH: HashOut<F> =
                hash_branch(&P1_EVENTS[1].hash(), &P2_EVENTS[1].hash());
            pub static ref MERGE_HASH: HashOut<F> = hash_branch(&MERGE_42_HASH, &MERGE_80_HASH);
            pub static ref MERGE_42: ProofWithPublicInputs<F, C, D> =
                merge_events(P1_EVENTS[0], P2_EVENTS[0]);
            pub static ref MERGE_80: ProofWithPublicInputs<F, C, D> =
                merge_events(P1_EVENTS[1], P2_EVENTS[1]);
            pub static ref MERGE_PROOF: ProofWithPublicInputs<F, C, D> =
                merge_merges(true, &MERGE_42, true, &MERGE_80);
        }
    }

    /// Helpers with P2 to the left of P1
    pub mod p2_p1 {
        use super::*;

        lazy_static! {
            pub static ref CAST_ROOT: HashOut<F> =
                hash_branch_bytes(&PROGRAM_2_HASH.into(), &PROGRAM_1_HASH.into());
        }

        lazy_static! {
            pub static ref PROGRAM_1_PROOF: ProofWithPublicInputs<F, C, D> = make_program(
                &PROGRAM_1,
                PROGRAM_1_HASH,
                Some(P1_BUILT_EVENTS.vm_hash),
                CALL_LIST_1,
                *CAST_ROOT
            );
            pub static ref PROGRAM_2_PROOF: ProofWithPublicInputs<F, C, D> = make_program(
                &PROGRAM_2,
                PROGRAM_2_HASH,
                Some(P2_BUILT_EVENTS.vm_hash),
                CALL_LIST_1,
                *CAST_ROOT
            );
        }

        lazy_static! {
            pub static ref MERGE_42_HASH: HashOut<F> =
                hash_branch(&P2_EVENTS[0].hash(), &P1_EVENTS[0].hash());
            pub static ref MERGE_80_HASH: HashOut<F> =
                hash_branch(&P2_EVENTS[1].hash(), &P1_EVENTS[1].hash());
            pub static ref MERGE_HASH: HashOut<F> = hash_branch(&MERGE_42_HASH, &MERGE_80_HASH);
            pub static ref MERGE_42: ProofWithPublicInputs<F, C, D> =
                merge_events(P2_EVENTS[0], P1_EVENTS[0]);
            pub static ref MERGE_80: ProofWithPublicInputs<F, C, D> =
                merge_events(P2_EVENTS[1], P1_EVENTS[1]);
            pub static ref MERGE_PROOF: ProofWithPublicInputs<F, C, D> =
                merge_merges(true, &MERGE_42, true, &MERGE_80);
        }
    }

    #[test]
    fn verify_leaf() -> Result<()> {
        let proof = LEAF.prove(
            &BRANCH,
            &PROGRAM_1.circuit.verifier_only,
            &p1_p2::PROGRAM_1_PROOF,
            &P1_BUILT_EVENTS.proof,
        )?;
        LEAF.circuit.verify(proof)?;

        let proof = LEAF.prove(
            &BRANCH,
            &PROGRAM_2.circuit.verifier_only,
            &p1_p2::PROGRAM_2_PROOF,
            &P2_BUILT_EVENTS.proof,
        )?;
        LEAF.circuit.verify(proof)?;

        let proof = LEAF.prove(
            &BRANCH,
            &PROGRAM_1.circuit.verifier_only,
            &p2_p1::PROGRAM_1_PROOF,
            &P1_BUILT_EVENTS.proof,
        )?;
        LEAF.circuit.verify(proof)?;

        let proof = LEAF.prove(
            &BRANCH,
            &PROGRAM_2.circuit.verifier_only,
            &p2_p1::PROGRAM_2_PROOF,
            &P2_BUILT_EVENTS.proof,
        )?;
        LEAF.circuit.verify(proof)?;

        Ok(())
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_leaf_wrong_verifier() {
        let proof = LEAF
            .prove(
                &BRANCH,
                &PROGRAM_2.circuit.verifier_only,
                &p1_p2::PROGRAM_1_PROOF,
                &P2_BUILT_EVENTS.proof,
            )
            .unwrap();
        LEAF.circuit.verify(proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_leaf_wrong_events_1() {
        let proof = LEAF
            .prove(
                &BRANCH,
                &PROGRAM_1.circuit.verifier_only,
                &p1_p2::PROGRAM_1_PROOF,
                &P2_BUILT_EVENTS.proof,
            )
            .unwrap();
        LEAF.circuit.verify(proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_leaf_wrong_events_2() {
        let proof = LEAF
            .prove(
                &BRANCH,
                &PROGRAM_2.circuit.verifier_only,
                &p1_p2::PROGRAM_2_PROOF,
                &P1_BUILT_EVENTS.proof,
            )
            .unwrap();
        LEAF.circuit.verify(proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_leaf_wrong_events_3() {
        let proof = LEAF
            .prove(
                &BRANCH,
                &PROGRAM_1.circuit.verifier_only,
                &p2_p1::PROGRAM_1_PROOF,
                &P2_BUILT_EVENTS.proof,
            )
            .unwrap();
        LEAF.circuit.verify(proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_leaf_wrong_events_4() {
        let proof = LEAF
            .prove(
                &BRANCH,
                &PROGRAM_2.circuit.verifier_only,
                &p2_p1::PROGRAM_2_PROOF,
                &P1_BUILT_EVENTS.proof,
            )
            .unwrap();
        LEAF.circuit.verify(proof).unwrap();
    }

    #[test]
    fn verify_branch() -> Result<()> {
        use p1_p2::{MERGE_PROOF, PROGRAM_1_PROOF, PROGRAM_2_PROOF};

        let leaf_1_proof = LEAF.prove(
            &BRANCH,
            &PROGRAM_1.circuit.verifier_only,
            &PROGRAM_1_PROOF,
            &P1_BUILT_EVENTS.proof,
        )?;
        LEAF.circuit.verify(leaf_1_proof.clone())?;

        let leaf_2_proof = LEAF.prove(
            &BRANCH,
            &PROGRAM_2.circuit.verifier_only,
            &PROGRAM_2_PROOF,
            &P2_BUILT_EVENTS.proof,
        )?;
        LEAF.circuit.verify(leaf_2_proof.clone())?;

        let branch_proof = BRANCH.prove(&MERGE_PROOF, true, &leaf_1_proof, true, &leaf_2_proof)?;
        BRANCH.circuit.verify(branch_proof.clone())?;

        Ok(())
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_branch_hash_merge_1() {
        let (merge_proof, leaf_1_proof, leaf_2_proof) = catch_unwind(|| {
            use p1_p2::{MERGE_80, PROGRAM_1_PROOF, PROGRAM_2_PROOF};
            // Flip the merge to break stuff
            use p2_p1::MERGE_42;

            let merge_proof = merge_merges(true, &MERGE_42, true, &MERGE_80);

            let leaf_1_proof = LEAF.prove(
                &BRANCH,
                &PROGRAM_1.circuit.verifier_only,
                &PROGRAM_1_PROOF,
                &P1_BUILT_EVENTS.proof,
            )?;
            LEAF.circuit.verify(leaf_1_proof.clone())?;

            let leaf_2_proof = LEAF.prove(
                &BRANCH,
                &PROGRAM_2.circuit.verifier_only,
                &PROGRAM_2_PROOF,
                &P2_BUILT_EVENTS.proof,
            )?;
            LEAF.circuit.verify(leaf_2_proof.clone())?;

            Result::<_>::Ok((merge_proof, leaf_1_proof, leaf_2_proof))
        })
        .expect("shouldn't fail")
        .unwrap();

        let branch_proof = BRANCH
            .prove(&merge_proof, true, &leaf_1_proof, true, &leaf_2_proof)
            .unwrap();
        BRANCH.circuit.verify(branch_proof.clone()).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_branch_hash_merge_2() {
        let (merge_proof, leaf_1_proof, leaf_2_proof) = catch_unwind(|| {
            use p1_p2::{MERGE_42, MERGE_80, PROGRAM_1_PROOF, PROGRAM_2_PROOF};

            // Flip the merge of the merge to break stuff
            let merge_proof = merge_merges(true, &MERGE_80, true, &MERGE_42);

            let leaf_1_proof = LEAF.prove(
                &BRANCH,
                &PROGRAM_1.circuit.verifier_only,
                &PROGRAM_1_PROOF,
                &P1_BUILT_EVENTS.proof,
            )?;
            LEAF.circuit.verify(leaf_1_proof.clone())?;

            let leaf_2_proof = LEAF.prove(
                &BRANCH,
                &PROGRAM_2.circuit.verifier_only,
                &PROGRAM_2_PROOF,
                &P2_BUILT_EVENTS.proof,
            )?;
            LEAF.circuit.verify(leaf_2_proof.clone())?;

            Result::<_>::Ok((merge_proof, leaf_1_proof, leaf_2_proof))
        })
        .expect("shouldn't fail")
        .unwrap();

        let branch_proof = BRANCH
            .prove(&merge_proof, true, &leaf_1_proof, true, &leaf_2_proof)
            .unwrap();
        BRANCH.circuit.verify(branch_proof.clone()).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_branch_call_list() {
        let (merge_proof, leaf_1_proof, leaf_2_proof) = catch_unwind(|| {
            // `PROGRAM_2B_PROOF` uses a different call list
            use p1_p2::{MERGE_42, MERGE_80, PROGRAM_1_PROOF, PROGRAM_2B_PROOF};

            let merge_proof = merge_merges(true, &MERGE_42, true, &MERGE_80);

            let leaf_1_proof = LEAF.prove(
                &BRANCH,
                &PROGRAM_1.circuit.verifier_only,
                &PROGRAM_1_PROOF,
                &P1_BUILT_EVENTS.proof,
            )?;
            LEAF.circuit.verify(leaf_1_proof.clone())?;

            let leaf_2_proof = LEAF.prove(
                &BRANCH,
                &PROGRAM_2.circuit.verifier_only,
                &PROGRAM_2B_PROOF,
                &P2_BUILT_EVENTS.proof,
            )?;
            LEAF.circuit.verify(leaf_2_proof.clone())?;

            Result::<_>::Ok((merge_proof, leaf_1_proof, leaf_2_proof))
        })
        .expect("shouldn't fail")
        .unwrap();

        let branch_proof = BRANCH
            .prove(&merge_proof, true, &leaf_1_proof, true, &leaf_2_proof)
            .unwrap();
        BRANCH.circuit.verify(branch_proof.clone()).unwrap();
    }
}

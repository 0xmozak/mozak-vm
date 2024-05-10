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
use crate::subcircuits::unpruned::PartialAllowed;
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
    pub program_id: unpruned::BranchSubCircuit<PartialAllowed>,

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

        // Connect the partials
        builder.connect(
            events_targets.partial.target,
            program_id_targets.extension.partial.target,
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
        right_proof: Option<(bool, &ProofWithPublicInputs<F, C, D>)>,
    ) -> Result<ProofWithPublicInputs<F, C, D>> {
        let mut inputs = PartialWitness::new();
        let partial = right_proof.is_none();
        let (right_is_leaf, right_proof) = right_proof.unwrap_or((left_is_leaf, left_proof));
        self.unbounded.set_witness(
            &mut inputs,
            left_is_leaf,
            left_proof,
            right_is_leaf,
            right_proof,
        );
        self.events.set_witness(&mut inputs, partial, merge);
        self.circuit.prove(inputs)
    }
}

#[cfg(test)]
pub mod test {
    use once_cell::sync::Lazy;
    use plonky2::field::types::Field;
    use plonky2::gates::noop::NoopGate;
    use plonky2::hash::hash_types::HashOutTarget;
    use plonky2::iop::target::{BoolTarget, Target};
    use plonky2::iop::witness::WitnessWrite;

    use self::core::ProgramPublicIndices;
    use super::*;
    use crate::circuits::build_event_root::test as build_event_root;
    use crate::circuits::merge::test as merge;
    use crate::circuits::test_data::{
        CALL_LISTS, CAST_PM_P0, CAST_PM_P1, CAST_ROOT, CAST_T0, CAST_T1, PROGRAM_HASHES, T0_HASH,
        T0_PM_P0_HASH, T1_B_HASH, T1_HASH, T1_P2_HASH,
    };
    use crate::indices::{ArrayTargetIndex, BoolTargetIndex, HashTargetIndex};
    use crate::test_utils::{C, CONFIG, D, F, NON_ZERO_VALUES, ZERO_VAL};

    pub struct DummyCircuit {
        /// The program hash
        pub program_hash_val: [F; 4],

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

    impl DummyCircuit {
        #[must_use]
        pub fn new(circuit_config: &CircuitConfig, program_id: impl Into<Option<usize>>) -> Self {
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

            let program_hash_val = program_id
                .into()
                .map_or(ZERO_VAL, |pid| PROGRAM_HASHES[pid]);

            let program_hash_calc = program_hash_val.map(|x| builder.constant(x));
            for (p, c) in program_hash.into_iter().zip(program_hash_calc) {
                builder.connect(p, c);
            }

            // Make sure we have enough gates to match.
            builder.add_gate(NoopGate, vec![]);
            while builder.num_gates() < (1 << 3) {
                builder.add_gate(NoopGate, vec![]);
            }

            let circuit = builder.build();

            Self {
                program_hash_val,
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
                program_hash: ArrayTargetIndex::new(public_inputs, &self.program_hash),
                events_present: BoolTargetIndex::new(public_inputs, self.events_present),
                event_root: HashTargetIndex::new(public_inputs, self.event_root),
                call_list: ArrayTargetIndex::new(public_inputs, &self.call_list),
                cast_root: HashTargetIndex::new(public_inputs, self.cast_root),
            }
        }

        pub fn prove(
            &self,
            event_root: Option<HashOut<F>>,
            call_list: [F; 4],
            cast_root: HashOut<F>,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            inputs.set_bool_target(self.events_present, event_root.is_some());
            inputs.set_hash_target(self.event_root, event_root.unwrap_or_default());
            inputs.set_target_arr(&self.call_list, &call_list);
            inputs.set_hash_target(self.cast_root, cast_root);
            self.circuit.prove(inputs)
        }
    }

    pub static PROGRAM_M: Lazy<DummyCircuit> = Lazy::new(|| DummyCircuit::new(&CONFIG, None));
    pub static PROGRAM_M_INDICES: Lazy<ProgramPublicIndices> =
        Lazy::new(|| PROGRAM_M.get_indices());
    pub static PROGRAM_0: Lazy<DummyCircuit> = Lazy::new(|| DummyCircuit::new(&CONFIG, 0));
    pub static PROGRAM_0_INDICES: Lazy<ProgramPublicIndices> =
        Lazy::new(|| PROGRAM_0.get_indices());
    pub static PROGRAM_1: Lazy<DummyCircuit> = Lazy::new(|| DummyCircuit::new(&CONFIG, 1));
    pub static PROGRAM_1_INDICES: Lazy<ProgramPublicIndices> =
        Lazy::new(|| PROGRAM_1.get_indices());
    pub static PROGRAM_2: Lazy<DummyCircuit> = Lazy::new(|| DummyCircuit::new(&CONFIG, 2));
    pub static PROGRAM_2_INDICES: Lazy<ProgramPublicIndices> =
        Lazy::new(|| PROGRAM_2.get_indices());

    #[tested_fixture::tested_fixture(pub LEAF)]
    fn build_leaf() -> LeafCircuit<F, C, D> {
        assert_eq!(*PROGRAM_M_INDICES, *PROGRAM_0_INDICES);
        assert_eq!(*PROGRAM_M_INDICES, *PROGRAM_1_INDICES);
        assert_eq!(*PROGRAM_M_INDICES, *PROGRAM_2_INDICES);

        assert_eq!(PROGRAM_M.circuit.common, PROGRAM_0.circuit.common);
        assert_eq!(PROGRAM_M.circuit.common, PROGRAM_0.circuit.common);
        assert_eq!(PROGRAM_M.circuit.common, PROGRAM_1.circuit.common);
        assert_eq!(PROGRAM_M.circuit.common, PROGRAM_2.circuit.common);

        LeafCircuit::new(
            &CONFIG,
            &PROGRAM_M_INDICES,
            &PROGRAM_M.circuit.common,
            &build_event_root::BRANCH,
        )
    }

    #[tested_fixture::tested_fixture(pub BRANCH)]
    fn build_branch() -> BranchCircuit<F, C, D> {
        BranchCircuit::new(&CONFIG, &merge::BRANCH, &LEAF)
    }

    fn assert_value(
        proof: &ProofWithPublicInputs<F, C, D>,
        event_hash: Option<HashOut<F>>,
        pid: [F; 4],
    ) {
        let indices = &LEAF.events.indices;
        assert_eq!(*indices, BRANCH.events.indices);

        let p_present = indices.hash_present.get_any(&proof.public_inputs);
        assert_eq!(p_present, F::from_bool(event_hash.is_some()));
        let p_hash = indices.hash.get_any(&proof.public_inputs);
        assert_eq!(p_hash, event_hash.unwrap_or_default().elements);

        let indices = &LEAF.program_id.indices;
        assert_eq!(*indices, BRANCH.program_id.indices);
        let p_pid = indices.unpruned_hash.get_any(&proof.public_inputs);
        assert_eq!(p_pid, pid);
    }

    #[allow(clippy::type_complexity)]
    fn verify_leaf(
        event_proof: &ProofWithPublicInputs<F, C, D>,
        hash: HashOut<F>,
        vm_hash: HashOut<F>,
        program: &'static DummyCircuit,
        program_verifier: &DummyCircuit,
        call_list: [F; 4],
        cast_root: HashOut<F>,
    ) -> Result<(ProofWithPublicInputs<F, C, D>, HashOut<F>, [F; 4])> {
        let program_proof = program
            .prove(Some(vm_hash), call_list, cast_root)
            .expect("shouldn't fail");
        program
            .circuit
            .verify(program_proof.clone())
            .expect("shouldn't fail");

        let proof = LEAF.prove(
            &BRANCH,
            &program_verifier.circuit.verifier_only,
            &program_proof,
            event_proof,
        )?;
        assert_value(&proof, Some(hash), program.program_hash_val);
        LEAF.circuit.verify(proof.clone())?;
        Ok((proof, hash, program.program_hash_val))
    }

    macro_rules! make_leaf_tests {
        ($($($name:ident | $proof:ident = ($event:ident, $program:ident, $tx:literal)),+ $(,)?)?) => {$($(
    #[tested_fixture::tested_fixture($proof: (ProofWithPublicInputs<F, C, D>, HashOut<F>, [F; 4]))]
    fn $name() -> Result<(ProofWithPublicInputs<F, C, D>, HashOut<F>, [F; 4])> {
        let &(ref event_proof, hash, vm_hash) = *build_event_root::$event;
        verify_leaf(
            event_proof,
            hash,
            vm_hash,
            &$program,
            &$program,
            CALL_LISTS[$tx],
            CAST_ROOT[$tx],
        )
    }
        )+)?};
    }

    make_leaf_tests! {
        verify_t0_pm_leaf | T0_PM_LEAF_PROOF = (T0_PM_BRANCH_PROOF, PROGRAM_M, 0),
        verify_t0_p0_leaf | T0_P0_LEAF_PROOF = (T0_P0_BRANCH_PROOF, PROGRAM_0, 0),
        verify_t0_p2_leaf | T0_P2_LEAF_PROOF = (T0_P2_BRANCH_PROOF, PROGRAM_2, 0),

        verify_t1_pm_leaf | T1_PM_LEAF_PROOF = (T1_PM_BRANCH_PROOF, PROGRAM_M, 1),
        verify_t1_p1_leaf | T1_P1_LEAF_PROOF = (T1_P1_BRANCH_PROOF, PROGRAM_1, 1),
        verify_t1_p2_leaf | T1_P2_LEAF_PROOF = (T1_P2_BRANCH_PROOF, PROGRAM_2, 1),
    }

    #[tested_fixture::tested_fixture(T0_PM_BAD_CALL_LEAF_PROOF: (ProofWithPublicInputs<F, C, D>, HashOut<F>, [F; 4]))]
    fn verify_t0_pm_bad_call_leaf() -> Result<(ProofWithPublicInputs<F, C, D>, HashOut<F>, [F; 4])>
    {
        let &(ref event_proof, hash, vm_hash) = *build_event_root::T0_PM_BRANCH_PROOF;
        verify_leaf(
            event_proof,
            hash,
            vm_hash,
            &PROGRAM_M,
            &PROGRAM_M,
            NON_ZERO_VALUES[0],
            CAST_ROOT[0],
        )
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_leaf_wrong_verifier() {
        let &(ref event_proof, hash, vm_hash) = *build_event_root::T0_PM_BRANCH_PROOF;

        verify_leaf(
            event_proof,
            hash,
            vm_hash,
            &PROGRAM_M,
            &PROGRAM_0,
            CALL_LISTS[0],
            CAST_ROOT[0],
        )
        .unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_leaf_wrong_proof() {
        let &(ref event_proof, hash, vm_hash) = *build_event_root::T0_PM_BRANCH_PROOF;

        verify_leaf(
            event_proof,
            hash,
            vm_hash,
            &PROGRAM_0,
            &PROGRAM_M,
            CALL_LISTS[0],
            CAST_ROOT[0],
        )
        .unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_leaf_wrong_events_1() {
        let (event_proof, _, _) = *build_event_root::T0_PM_BRANCH_PROOF;
        let &(_, hash, vm_hash) = *build_event_root::T1_PM_BRANCH_PROOF;

        verify_leaf(
            event_proof,
            hash,
            vm_hash,
            &PROGRAM_M,
            &PROGRAM_M,
            CALL_LISTS[0],
            CAST_ROOT[0],
        )
        .unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_leaf_wrong_events_2() {
        let (event_proof, _, _) = *build_event_root::T0_PM_BRANCH_PROOF;
        let &(_, hash, vm_hash) = *build_event_root::T0_P2_BRANCH_PROOF;

        verify_leaf(
            event_proof,
            hash,
            vm_hash,
            &PROGRAM_M,
            &PROGRAM_M,
            CALL_LISTS[0],
            CAST_ROOT[0],
        )
        .unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_leaf_wrong_events_3() {
        let &(ref event_proof, hash, _) = *build_event_root::T0_PM_BRANCH_PROOF;
        let &(_, _, vm_hash) = *build_event_root::T1_PM_BRANCH_PROOF;

        verify_leaf(
            event_proof,
            hash,
            vm_hash,
            &PROGRAM_M,
            &PROGRAM_M,
            CALL_LISTS[0],
            CAST_ROOT[0],
        )
        .unwrap();
    }

    #[test]
    #[should_panic(expected = "assertion `left == right`")]
    fn bad_leaf_wrong_events_4() {
        let &(ref event_proof, _, vm_hash) = *build_event_root::T0_PM_BRANCH_PROOF;
        let &(_, hash, _) = *build_event_root::T1_PM_BRANCH_PROOF;

        verify_leaf(
            event_proof,
            hash,
            vm_hash,
            &PROGRAM_M,
            &PROGRAM_M,
            CALL_LISTS[0],
            CAST_ROOT[0],
        )
        .unwrap();
    }

    #[tested_fixture::tested_fixture(T0_PM_P0_BRANCH_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_t0_pm_p0_branch() -> Result<ProofWithPublicInputs<F, C, D>> {
        let proof = BRANCH.prove(
            *merge::T0_PM_P0_BRANCH_PROOF,
            true,
            &T0_PM_LEAF_PROOF.0,
            Some((true, &T0_P0_LEAF_PROOF.0)),
        )?;
        assert_value(&proof, Some(*T0_PM_P0_HASH), *CAST_PM_P0);
        BRANCH.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(pub T0_BRANCH_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_t0_branch() -> Result<ProofWithPublicInputs<F, C, D>> {
        let proof = BRANCH.prove(
            *merge::T0_BRANCH_PROOF,
            false,
            *T0_PM_P0_BRANCH_PROOF,
            Some((true, &T0_P2_LEAF_PROOF.0)),
        )?;
        assert_value(&proof, Some(*T0_HASH), *CAST_T0);
        BRANCH.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(T1_PM_P1_BRANCH_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_t1_pm_p1_branch() -> Result<ProofWithPublicInputs<F, C, D>> {
        let proof = BRANCH.prove(
            *merge::T1_PM_P1_BRANCH_PROOF,
            true,
            &T1_PM_LEAF_PROOF.0,
            Some((true, &T1_P1_LEAF_PROOF.0)),
        )?;
        assert_value(&proof, Some(*T1_B_HASH), *CAST_PM_P1);
        BRANCH.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(pub T1_BRANCH_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_t1_branch() -> Result<ProofWithPublicInputs<F, C, D>> {
        let proof = BRANCH.prove(
            *merge::T1_BRANCH_PROOF,
            false,
            &T1_PM_P1_BRANCH_PROOF,
            Some((true, &T1_P2_LEAF_PROOF.0)),
        )?;
        assert_value(&proof, Some(*T1_HASH), *CAST_T1);
        BRANCH.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[test]
    fn verify_partial_branch_1() -> Result<()> {
        let proof = BRANCH.prove(
            *merge::T1_P2_PARTIAL_BRANCH_PROOF,
            true,
            &T1_P2_LEAF_PROOF.0,
            None,
        )?;
        assert_value(&proof, Some(*T1_P2_HASH), PROGRAM_HASHES[2]);
        BRANCH.circuit.verify(proof)?;
        Ok(())
    }

    #[test]
    fn verify_partial_branch_2() -> Result<()> {
        let proof = BRANCH.prove(
            *merge::T1_PM_P1_PARTIAL_BRANCH_PROOF,
            false,
            &T1_PM_P1_BRANCH_PROOF,
            None,
        )?;
        assert_value(&proof, Some(*T1_B_HASH), *CAST_PM_P1);
        BRANCH.circuit.verify(proof)?;
        Ok(())
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_branch_hash_merge_1() {
        let proof = BRANCH
            .prove(
                *merge::T0_PM_P0_BRANCH_PROOF,
                // Flip the merge to break stuff
                true,
                &T0_P0_LEAF_PROOF.0,
                Some((true, &T0_PM_LEAF_PROOF.0)),
            )
            .unwrap();
        BRANCH.circuit.verify(proof.clone()).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_branch_hash_merge_2() {
        let proof = BRANCH
            .prove(
                *merge::T0_BRANCH_PROOF,
                // Flip the merge to break stuff
                true,
                &T0_P2_LEAF_PROOF.0,
                Some((false, *T0_PM_P0_BRANCH_PROOF)),
            )
            .unwrap();
        BRANCH.circuit.verify(proof.clone()).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_branch_call_list() {
        let proof = BRANCH
            .prove(
                *merge::T0_PM_P0_BRANCH_PROOF,
                true,
                &T0_PM_BAD_CALL_LEAF_PROOF.0,
                Some((true, &T0_P0_LEAF_PROOF.0)),
            )
            .unwrap();
        BRANCH.circuit.verify(proof.clone()).unwrap();
    }
}

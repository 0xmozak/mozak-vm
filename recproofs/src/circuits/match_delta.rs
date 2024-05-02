//! Circuits for recursively proving state delta objects match summarized state
//! updates

use anyhow::Result;
use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::witness::{PartialWitness, WitnessWrite};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::{CircuitConfig, CircuitData};
use plonky2::plonk::config::{AlgebraicHasher, GenericConfig};
use plonky2::plonk::proof::{ProofWithPublicInputs, ProofWithPublicInputsTarget};

use super::accumulate_delta;
use crate::circuits::accumulate_delta::core::SplitFlags;
use crate::maybe_connect;
use crate::subcircuits::unbounded;
use crate::subcircuits::unpruned::{self, PartialAllowed};

// The core subcircuit for this circuit
pub mod core;

pub struct LeafTargets<const D: usize> {
    /// The proof of event accumulation
    pub accumulate_event: ProofWithPublicInputsTarget<D>,
}

pub struct LeafCircuit<F, C, const D: usize>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>, {
    /// The recursion subcircuit
    pub unbounded: unbounded::LeafSubCircuit,

    /// The rp-style merkle hash of events
    pub event_hash: unpruned::LeafSubCircuit,

    /// The rp-style merkle hash of summarized state updates
    pub state_hash: unpruned::LeafSubCircuit,

    /// The delta/state update comparator
    pub compare_delta: core::LeafSubCircuit,

    pub targets: LeafTargets<D>,

    pub circuit: CircuitData<F, C, D>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct LeafWitnessValue<F> {
    block_height: u64,
    last_updated: u64,
    old_owner: [F; 4],
    new_owner: [F; 4],
    old_data: [F; 4],
    new_data: [F; 4],
    old_credits: u64,
    new_credits: u64,
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
        accumulate_event_circuit: &accumulate_delta::BranchCircuit<F, C, D>,
    ) -> Self {
        let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());

        let unbounded_inputs = unbounded::SubCircuitInputs::default(&mut builder);
        let event_hash_inputs = unpruned::SubCircuitInputs::default(&mut builder);
        let state_hash_inputs = unpruned::SubCircuitInputs::default(&mut builder);
        let compare_delta_inputs = core::SubCircuitInputs::default(&mut builder);

        let unbounded_targets = unbounded_inputs.build_leaf::<F, C, D>(&mut builder);
        let event_hash_targets = event_hash_inputs.build_leaf(&mut builder);
        let state_hash_targets = state_hash_inputs.build_leaf(&mut builder);
        let compare_delta_targets = compare_delta_inputs.build_leaf(&mut builder);

        let targets = LeafTargets {
            accumulate_event: builder
                .add_virtual_proof_with_pis(&accumulate_event_circuit.circuit.common),
        };

        // Validate the accumulate_event proof and connect all the inputs
        let verifier =
            builder.constant_verifier_data(&accumulate_event_circuit.circuit.verifier_only);
        builder.verify_proof::<C>(
            &targets.accumulate_event,
            &verifier,
            &accumulate_event_circuit.circuit.common,
        );
        // event hash
        {
            let acc_event_hash = accumulate_event_circuit
                .event_hash
                .indices
                .unpruned_hash
                .get(&targets.accumulate_event.public_inputs);
            builder.connect_hashes(event_hash_targets.inputs.unpruned_hash, acc_event_hash);
        }
        // address
        {
            let acc_addr = accumulate_event_circuit
                .partial_state
                .indices
                .address
                .get(&targets.accumulate_event.public_inputs);
            builder.connect(compare_delta_targets.address, acc_addr);
        }
        // flags
        let acc_flags = accumulate_event_circuit
            .partial_state
            .indices
            .object_flags
            .get(&targets.accumulate_event.public_inputs);
        builder.connect(compare_delta_targets.object_flags, acc_flags);
        let acc_flags = SplitFlags::split(&mut builder, acc_flags);
        let has_owner = builder.is_nonzero(acc_flags.owner);
        // old owner
        {
            let acc_old_owner = accumulate_event_circuit
                .partial_state
                .indices
                .old_owner
                .get(&targets.accumulate_event.public_inputs);
            maybe_connect(
                &mut builder,
                compare_delta_targets.old_owner,
                has_owner,
                acc_old_owner,
            );
        }
        // new owner
        {
            let acc_new_owner = accumulate_event_circuit
                .partial_state
                .indices
                .new_owner
                .get(&targets.accumulate_event.public_inputs);
            maybe_connect(
                &mut builder,
                compare_delta_targets.new_owner,
                has_owner,
                acc_new_owner,
            );
        }
        // old data
        {
            let acc_old_data = accumulate_event_circuit
                .partial_state
                .indices
                .old_data
                .get(&targets.accumulate_event.public_inputs);
            maybe_connect(
                &mut builder,
                compare_delta_targets.old_data,
                acc_flags.read,
                acc_old_data,
            );
        }
        // new data
        {
            let acc_new_data = accumulate_event_circuit
                .partial_state
                .indices
                .new_data
                .get(&targets.accumulate_event.public_inputs);
            maybe_connect(
                &mut builder,
                compare_delta_targets.new_data,
                acc_flags.write,
                acc_new_data,
            );
        }
        // credit delta
        {
            let acc_credit_delta = accumulate_event_circuit
                .partial_state
                .indices
                .credit_delta
                .get(&targets.accumulate_event.public_inputs);
            let calc_credit_delta = builder.sub(
                compare_delta_targets.new_credits,
                compare_delta_targets.old_credits,
            );
            builder.connect(calc_credit_delta, acc_credit_delta);
        }

        // Connect the state hash
        builder.connect_hashes(
            compare_delta_targets.state_hash,
            state_hash_targets.inputs.unpruned_hash,
        );

        let circuit = builder.build();

        let public_inputs = &circuit.prover_only.public_inputs;
        let unbounded = unbounded_targets.build(public_inputs);
        let event_hash = event_hash_targets.build(public_inputs);
        let state_hash = state_hash_targets.build(public_inputs);
        let compare_delta = compare_delta_targets.build(public_inputs);

        Self {
            unbounded,
            event_hash,
            state_hash,
            compare_delta,
            targets,
            circuit,
        }
    }

    pub fn prove(
        &self,
        branch: &BranchCircuit<F, C, D>,
        accumulate_event_proof: ProofWithPublicInputs<F, C, D>,
        v: LeafWitnessValue<F>,
    ) -> Result<ProofWithPublicInputs<F, C, D>> {
        let mut inputs = PartialWitness::new();
        self.unbounded.set_witness(&mut inputs, &branch.circuit);
        inputs.set_target(
            self.compare_delta.targets.block_height,
            F::from_canonical_u64(v.block_height),
        );
        inputs.set_target(
            self.compare_delta.targets.last_updated,
            F::from_canonical_u64(v.last_updated),
        );
        inputs.set_target(
            self.compare_delta.targets.old_credits,
            F::from_canonical_u64(v.old_credits),
        );
        inputs.set_target(
            self.compare_delta.targets.new_credits,
            F::from_canonical_u64(v.new_credits),
        );
        inputs.set_target_arr(&self.compare_delta.targets.old_owner, &v.old_owner);
        inputs.set_target_arr(&self.compare_delta.targets.new_owner, &v.new_owner);
        inputs.set_target_arr(&self.compare_delta.targets.old_data, &v.old_data);
        inputs.set_target_arr(&self.compare_delta.targets.new_data, &v.new_data);
        inputs.set_proof_with_pis_target(&self.targets.accumulate_event, &accumulate_event_proof);
        self.circuit.prove(inputs)
    }
}

pub struct BranchCircuit<F, C, const D: usize>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>, {
    /// The recursion subcircuit
    pub unbounded: unbounded::BranchSubCircuit<D>,

    /// The rp-style merkle hash of events
    pub event_hash: unpruned::BranchSubCircuit<PartialAllowed>,

    /// The rp-style merkle hash of summarized state updates
    pub state_hash: unpruned::BranchSubCircuit<PartialAllowed>,

    /// The partial-object/state update comparator
    pub compare_delta: core::BranchSubCircuit,

    pub circuit: CircuitData<F, C, D>,
}

impl<F, C, const D: usize> BranchCircuit<F, C, D>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    <C as GenericConfig<D>>::Hasher: AlgebraicHasher<F>,
{
    #[must_use]
    pub fn new(circuit_config: &CircuitConfig, leaf: &LeafCircuit<F, C, D>) -> Self {
        let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());

        let unbounded_inputs = unbounded::SubCircuitInputs::default(&mut builder);
        let event_hash_inputs = unpruned::SubCircuitInputs::default(&mut builder);
        let state_hash_inputs = unpruned::SubCircuitInputs::default(&mut builder);
        let compare_delta_inputs = core::SubCircuitInputs::default(&mut builder);

        let unbounded_targets =
            unbounded_inputs.build_branch(&mut builder, &leaf.unbounded, &leaf.circuit);
        let event_hash_targets = event_hash_inputs.build_extended_branch(
            &mut builder,
            &leaf.event_hash.indices,
            &unbounded_targets.left_proof,
            &unbounded_targets.right_proof,
            false,
        );
        let state_hash_targets = state_hash_inputs.build_extended_branch(
            &mut builder,
            &leaf.state_hash.indices,
            &unbounded_targets.left_proof,
            &unbounded_targets.right_proof,
            false,
        );
        let compare_delta_targets = compare_delta_inputs.build_branch(
            &mut builder,
            &leaf.compare_delta.indices,
            &unbounded_targets.left_proof,
            &unbounded_targets.right_proof,
        );

        // Connect partials
        builder.connect(
            event_hash_targets.extension.partial.target,
            state_hash_targets.extension.partial.target,
        );

        let circuit = builder.build();

        let public_inputs = &circuit.prover_only.public_inputs;
        let unbounded = unbounded_targets.build(&leaf.unbounded, public_inputs);
        let event_hash = event_hash_targets.build(&leaf.event_hash.indices, public_inputs);
        let state_hash = state_hash_targets.build(&leaf.state_hash.indices, public_inputs);
        let compare_delta = compare_delta_targets.build(&leaf.compare_delta.indices, public_inputs);

        Self {
            unbounded,
            event_hash,
            state_hash,
            compare_delta,
            circuit,
        }
    }

    pub fn prove(
        &self,
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
        self.event_hash.set_witness(&mut inputs, None, partial);
        self.state_hash.set_witness(&mut inputs, None, partial);
        self.circuit.prove(inputs)
    }
}

#[cfg(test)]
pub mod test {
    use lazy_static::lazy_static;
    use plonky2::field::types::Field;

    use super::*;
    use crate::circuits::accumulate_delta::test::{BRANCH as ACC_BRANCH, LEAF as ACC_LEAF};
    use crate::test_utils::{C, CONFIG, D, F};
    use crate::EventType;

    lazy_static! {
        pub static ref LEAF: LeafCircuit<F, C, D> = LeafCircuit::new(&CONFIG, &ACC_BRANCH);
        pub static ref BRANCH: BranchCircuit<F, C, D> = BranchCircuit::new(&CONFIG, &LEAF);
    }

    fn make_acc_proof(
        vals: impl IntoIterator<Item = (u64, [F; 4], EventType, [F; 4])>,
    ) -> Result<ProofWithPublicInputs<F, C, D>> {
        let mut vals = vals.into_iter();
        let val = vals.next().unwrap();
        let mut acc_left_proof = ACC_LEAF.prove(&ACC_BRANCH, val.0, val.1, val.2, val.3)?;
        ACC_LEAF.circuit.verify(acc_left_proof.clone())?;
        let mut left_is_leaf = true;
        for val in vals {
            let acc_right_proof = ACC_LEAF.prove(&ACC_BRANCH, val.0, val.1, val.2, val.3)?;
            ACC_LEAF.circuit.verify(acc_right_proof.clone())?;

            acc_left_proof = ACC_BRANCH.prove(
                left_is_leaf,
                &acc_left_proof,
                Some((true, &acc_right_proof)),
            )?;
            ACC_BRANCH.circuit.verify(acc_left_proof.clone())?;

            left_is_leaf = false;
        }

        acc_left_proof = ACC_BRANCH.prove(left_is_leaf, &acc_left_proof, None)?;
        ACC_BRANCH.circuit.verify(acc_left_proof.clone())?;
        Ok(acc_left_proof)
    }

    #[test]
    fn verify_leaf() -> Result<()> {
        let program_hash_1 = [4, 8, 15, 16].map(F::from_canonical_u64);

        let non_zero_val_1 = [3, 1, 4, 15].map(F::from_canonical_u64);
        let non_zero_val_2 = [1, 6, 180, 33].map(F::from_canonical_u64);

        let acc_branch_proof_1 = make_acc_proof([
            (200, program_hash_1, EventType::Read, non_zero_val_1),
            (200, program_hash_1, EventType::Write, non_zero_val_2),
            (200, program_hash_1, EventType::Ensure, non_zero_val_2),
        ])?;

        let leaf_proof = LEAF.prove(&BRANCH, acc_branch_proof_1, LeafWitnessValue {
            block_height: 10,
            last_updated: 9,
            old_owner: program_hash_1,
            new_owner: program_hash_1,
            old_data: non_zero_val_1,
            new_data: non_zero_val_2,
            old_credits: 50,
            new_credits: 50,
        })?;
        LEAF.circuit.verify(leaf_proof)?;

        Ok(())
    }

    #[test]
    fn verify_branch() -> Result<()> {
        let program_hash_1 = [4, 8, 15, 16].map(F::from_canonical_u64);

        let non_zero_val_1 = [3, 1, 4, 15].map(F::from_canonical_u64);
        let non_zero_val_2 = [1, 6, 180, 33].map(F::from_canonical_u64);

        let acc_branch_proof_200 = make_acc_proof([
            (200, program_hash_1, EventType::Read, non_zero_val_1),
            (200, program_hash_1, EventType::Write, non_zero_val_2),
            (200, program_hash_1, EventType::Ensure, non_zero_val_2),
        ])?;

        let acc_branch_proof_400 = make_acc_proof([
            (400, program_hash_1, EventType::Read, non_zero_val_1),
            (400, program_hash_1, EventType::Write, non_zero_val_2),
            (400, program_hash_1, EventType::Ensure, non_zero_val_2),
        ])?;

        let leaf_proof_200 = LEAF.prove(&BRANCH, acc_branch_proof_200, LeafWitnessValue {
            block_height: 10,
            last_updated: 9,
            old_owner: program_hash_1,
            new_owner: program_hash_1,
            old_data: non_zero_val_1,
            new_data: non_zero_val_2,
            old_credits: 50,
            new_credits: 50,
        })?;
        LEAF.circuit.verify(leaf_proof_200.clone())?;

        let leaf_proof_400 = LEAF.prove(&BRANCH, acc_branch_proof_400, LeafWitnessValue {
            block_height: 10,
            last_updated: 9,
            old_owner: program_hash_1,
            new_owner: program_hash_1,
            old_data: non_zero_val_1,
            new_data: non_zero_val_2,
            old_credits: 50,
            new_credits: 50,
        })?;
        LEAF.circuit.verify(leaf_proof_400.clone())?;

        let branch_proof_1 = BRANCH.prove(true, &leaf_proof_200, Some((true, &leaf_proof_400)))?;
        BRANCH.circuit.verify(branch_proof_1.clone())?;

        let branch_proof_2 = BRANCH.prove(false, &branch_proof_1, None)?;
        BRANCH.circuit.verify(branch_proof_2.clone())?;

        Ok(())
    }
}

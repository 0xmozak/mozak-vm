//! Circuits for proving events can be accumulated to a state delta object.

use anyhow::Result;
use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::witness::PartialWitness;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::{CircuitConfig, CircuitData};
use plonky2::plonk::config::{AlgebraicHasher, GenericConfig};
use plonky2::plonk::proof::ProofWithPublicInputs;

use crate::subcircuits::unpruned::PartialAllowed;
use crate::subcircuits::{unbounded, unpruned};
use crate::{hash_event, Event, EventType};

// The core subcircuit for this circuit
pub mod core;

pub struct LeafCircuit<F, C, const D: usize>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>, {
    /// The recursion subcircuit
    pub unbounded: unbounded::LeafSubCircuit,

    /// The rp-style merkle hash of all event fields
    pub event_hash: unpruned::LeafSubCircuit,

    /// The event-to-state/partial-object translator
    pub partial_state: core::LeafSubCircuit,

    pub circuit: CircuitData<F, C, D>,
}

impl<F, C, const D: usize> LeafCircuit<F, C, D>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    <C as GenericConfig<D>>::Hasher: AlgebraicHasher<F>,
{
    #[must_use]
    pub fn new(circuit_config: &CircuitConfig) -> Self {
        let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());

        let unbounded_inputs = unbounded::SubCircuitInputs::default(&mut builder);
        let event_hash_inputs = unpruned::SubCircuitInputs::default(&mut builder);
        let partial_state_inputs = core::SubCircuitInputs::default(&mut builder);

        let unbounded_targets = unbounded_inputs.build_leaf::<F, C, D>(&mut builder);
        let event_hash_targets = event_hash_inputs.build_leaf(&mut builder);
        let partial_state_targets = partial_state_inputs.build_leaf(&mut builder);

        let event_hash_calc = hash_event(
            &mut builder,
            partial_state_targets.event_owner,
            partial_state_targets.event_ty,
            partial_state_targets.inputs.address,
            partial_state_targets.event_value,
        );
        builder.connect_hashes(event_hash_calc, event_hash_targets.inputs.unpruned_hash);

        let circuit = builder.build();

        let public_inputs = &circuit.prover_only.public_inputs;
        let unbounded = unbounded_targets.build(public_inputs);
        let event_hash = event_hash_targets.build(public_inputs);
        let partial_state = partial_state_targets.build(public_inputs);

        Self {
            unbounded,
            event_hash,
            partial_state,
            circuit,
        }
    }

    pub fn prove(
        &self,
        branch: &BranchCircuit<F, C, D>,
        address: u64,
        event_owner: [F; 4],
        event_ty: EventType,
        event_value: [F; 4],
    ) -> Result<ProofWithPublicInputs<F, C, D>> {
        let mut inputs = PartialWitness::new();
        self.unbounded.set_witness(&mut inputs, &branch.circuit);
        self.partial_state.set_witness(&mut inputs, Event {
            owner: event_owner,
            ty: event_ty,
            address,
            value: event_value,
        });
        self.circuit.prove(inputs)
    }
}

pub struct BranchCircuit<F, C, const D: usize>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>, {
    /// The recursion subcircuit
    pub unbounded: unbounded::BranchSubCircuit<D>,

    /// The rp-style merkle hash of all event fields
    pub event_hash: unpruned::BranchSubCircuit<PartialAllowed>,

    /// The event-to-state/partial-object translator
    pub partial_state: core::BranchSubCircuit,

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
        let partial_state_inputs = core::SubCircuitInputs::default(&mut builder);

        let unbounded_targets =
            unbounded_inputs.build_branch(&mut builder, &leaf.unbounded, &leaf.circuit);
        let event_hash_targets = event_hash_inputs.build_extended_branch(
            &mut builder,
            &leaf.event_hash.indices,
            &unbounded_targets.left_proof,
            &unbounded_targets.right_proof,
            false,
        );
        let partial_state_targets = partial_state_inputs.build_branch(
            &mut builder,
            &leaf.partial_state.indices,
            &unbounded_targets.left_proof,
            &unbounded_targets.right_proof,
        );

        builder.connect(
            event_hash_targets.extension.partial.target,
            partial_state_targets.partial.target,
        );

        let circuit = builder.build();

        let public_inputs = &circuit.prover_only.public_inputs;
        let unbounded = unbounded_targets.build(&leaf.unbounded, public_inputs);
        let event_hash = event_hash_targets.build(&leaf.event_hash.indices, public_inputs);
        let partial_state = partial_state_targets.build(&leaf.partial_state.indices, public_inputs);

        Self {
            unbounded,
            event_hash,
            partial_state,
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
        self.event_hash.set_witness(&mut inputs, partial);
        if partial {
            self.partial_state
                .set_witness_from_proof(&mut inputs, &left_proof.public_inputs);
        } else {
            self.partial_state.set_witness_from_proofs(
                &mut inputs,
                &left_proof.public_inputs,
                &right_proof.public_inputs,
            );
        }
        self.circuit.prove(inputs)
    }
}

#[cfg(test)]
pub mod test {
    use enumflags2::BitFlags;
    use plonky2::field::types::Field;
    use plonky2::hash::hash_types::HashOut;

    use super::*;
    use crate::circuits::test_data::{
        ADDRESS_A, ADDRESS_B, ADDRESS_C, ADDRESS_D, EVENT_T0_P0_A_CREDIT, EVENT_T0_P0_A_WRITE,
        EVENT_T0_P2_A_ENSURE, EVENT_T0_P2_A_READ, EVENT_T0_P2_C_TAKE, EVENT_T0_PM_C_CREDIT,
        EVENT_T0_PM_C_GIVE, EVENT_T0_PM_C_WRITE, EVENT_T1_P1_B_CREDIT, EVENT_T1_P1_B_GIVE,
        EVENT_T1_P1_B_WRITE, EVENT_T1_P2_A_READ, EVENT_T1_P2_D_READ, EVENT_T1_PM_B_ENSURE,
        EVENT_T1_PM_B_TAKE, STATE_0, STATE_1, T0_A_HASH, T0_C_HASH, T0_P0_HASH, T0_P2_A_HASH,
        T0_PM_C_CREDIT_GIVE_HASH, T0_PM_HASH, T0_T1_A_HASH, T1_B_HASH, T1_P1_B_WRITE_GIVE_HASH,
        T1_P1_HASH, T1_P2_D_HASH, T1_PM_HASH,
    };
    use crate::test_utils::{C, CONFIG, D, F};
    use crate::EventFlags;

    #[tested_fixture::tested_fixture(pub LEAF)]
    fn build_leaf() -> LeafCircuit<F, C, D> { LeafCircuit::new(&CONFIG) }

    #[tested_fixture::tested_fixture(pub BRANCH)]
    fn build_branch() -> BranchCircuit<F, C, D> { BranchCircuit::new(&CONFIG, &LEAF) }

    #[allow(clippy::too_many_arguments)]
    fn assert_proof(
        proof: &ProofWithPublicInputs<F, C, D>,
        event_hash: HashOut<F>,
        address: u64,
        flags: impl Into<BitFlags<EventFlags>>,
        old_owner: impl Into<Option<[F; 4]>>,
        new_owner: impl Into<Option<[F; 4]>>,
        old_data: impl Into<Option<[F; 4]>>,
        new_data: impl Into<Option<[F; 4]>>,
        credit_delta: impl Into<Option<F>>,
    ) {
        let indices = &LEAF.partial_state.indices;
        assert_eq!(*indices, BRANCH.partial_state.indices);

        let p_address = indices.address.get(&proof.public_inputs);
        assert_eq!(p_address, F::from_canonical_u64(address));
        let p_flags = indices.object_flags.get(&proof.public_inputs);
        assert_eq!(p_flags, F::from_canonical_u8(flags.into().bits()));
        let p_old_owner = indices.old_owner.get_any(&proof.public_inputs);
        assert_eq!(p_old_owner, old_owner.into().unwrap_or_default());
        let p_new_owner = indices.new_owner.get_any(&proof.public_inputs);
        assert_eq!(p_new_owner, new_owner.into().unwrap_or_default());
        let p_old_data = indices.old_data.get_any(&proof.public_inputs);
        assert_eq!(p_old_data, old_data.into().unwrap_or_default());
        let p_new_data = indices.new_data.get_any(&proof.public_inputs);
        assert_eq!(p_new_data, new_data.into().unwrap_or_default());
        let p_credit_delta = indices.credit_delta.get(&proof.public_inputs);
        assert_eq!(p_credit_delta, credit_delta.into().unwrap_or_default());

        let indices = &LEAF.event_hash.indices;
        assert_eq!(*indices, BRANCH.event_hash.indices);

        let p_event_hash = indices.unpruned_hash.get_any(&proof.public_inputs);
        assert_eq!(p_event_hash, event_hash.elements);
    }

    #[tested_fixture::tested_fixture(T0_PM_C_CREDIT_LEAF_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_t0_pm_c_credit_leaf() -> Result<ProofWithPublicInputs<F, C, D>> {
        let event = EVENT_T0_PM_C_CREDIT;
        let proof = LEAF.prove(&BRANCH, event.address, event.owner, event.ty, event.value)?;
        assert_proof(
            &proof,
            event.hash(),
            event.address,
            BitFlags::empty(),
            event.owner,
            None,
            None,
            None,
            event.value[0],
        );
        LEAF.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(T0_PM_C_GIVE_LEAF_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_t0_pm_c_give_leaf() -> Result<ProofWithPublicInputs<F, C, D>> {
        let event = EVENT_T0_PM_C_GIVE;
        let proof = LEAF.prove(&BRANCH, event.address, event.owner, event.ty, event.value)?;
        assert_proof(
            &proof,
            event.hash(),
            event.address,
            EventFlags::GiveOwnerFlag,
            event.owner,
            event.value,
            None,
            None,
            None,
        );
        LEAF.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(T0_PM_C_WRITE_LEAF_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_t0_pm_c_write_leaf() -> Result<ProofWithPublicInputs<F, C, D>> {
        let event = EVENT_T0_PM_C_WRITE;
        let proof = LEAF.prove(&BRANCH, event.address, event.owner, event.ty, event.value)?;
        assert_proof(
            &proof,
            event.hash(),
            event.address,
            EventFlags::WriteFlag,
            event.owner,
            None,
            None,
            event.value,
            None,
        );
        LEAF.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(T0_P0_A_WRITE_LEAF_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_t0_p0_a_write_leaf() -> Result<ProofWithPublicInputs<F, C, D>> {
        let event = EVENT_T0_P0_A_WRITE;
        let proof = LEAF.prove(&BRANCH, event.address, event.owner, event.ty, event.value)?;
        assert_proof(
            &proof,
            event.hash(),
            event.address,
            EventFlags::WriteFlag,
            event.owner,
            None,
            None,
            event.value,
            None,
        );
        LEAF.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(T0_P0_A_CREDIT_LEAF_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_t0_p0_a_credit_leaf() -> Result<ProofWithPublicInputs<F, C, D>> {
        let event = EVENT_T0_P0_A_CREDIT;
        let proof = LEAF.prove(&BRANCH, event.address, event.owner, event.ty, event.value)?;
        assert_proof(
            &proof,
            event.hash(),
            event.address,
            BitFlags::empty(),
            event.owner,
            None,
            None,
            None,
            -event.value[0],
        );
        LEAF.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(T0_P2_A_READ_LEAF_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_t0_p2_a_read_leaf() -> Result<ProofWithPublicInputs<F, C, D>> {
        let event = EVENT_T0_P2_A_READ;
        let proof = LEAF.prove(&BRANCH, event.address, event.owner, event.ty, event.value)?;
        assert_proof(
            &proof,
            event.hash(),
            event.address,
            EventFlags::ReadFlag,
            None,
            None,
            event.value,
            None,
            None,
        );
        LEAF.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(T0_P2_A_ENSURE_LEAF_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_t0_p2_a_ensure_leaf() -> Result<ProofWithPublicInputs<F, C, D>> {
        let event = EVENT_T0_P2_A_ENSURE;
        let proof = LEAF.prove(&BRANCH, event.address, event.owner, event.ty, event.value)?;
        assert_proof(
            &proof,
            event.hash(),
            event.address,
            EventFlags::EnsureFlag,
            None,
            None,
            None,
            event.value,
            None,
        );
        LEAF.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(T0_P2_C_TAKE_LEAF_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_t0_p2_c_take_leaf() -> Result<ProofWithPublicInputs<F, C, D>> {
        let event = EVENT_T0_P2_C_TAKE;
        let proof = LEAF.prove(&BRANCH, event.address, event.owner, event.ty, event.value)?;
        assert_proof(
            &proof,
            event.hash(),
            event.address,
            EventFlags::TakeOwnerFlag,
            event.value,
            event.owner,
            None,
            None,
            None,
        );
        LEAF.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(T1_PM_B_TAKE_LEAF_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_t1_pm_b_take_leaf() -> Result<ProofWithPublicInputs<F, C, D>> {
        let event = EVENT_T1_PM_B_TAKE;
        let proof = LEAF.prove(&BRANCH, event.address, event.owner, event.ty, event.value)?;
        assert_proof(
            &proof,
            event.hash(),
            event.address,
            EventFlags::TakeOwnerFlag,
            event.value,
            event.owner,
            None,
            None,
            None,
        );
        LEAF.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(T1_PM_B_ENSURE_LEAF_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_t1_pm_b_ensure_leaf() -> Result<ProofWithPublicInputs<F, C, D>> {
        let event = EVENT_T1_PM_B_ENSURE;
        let proof = LEAF.prove(&BRANCH, event.address, event.owner, event.ty, event.value)?;
        assert_proof(
            &proof,
            event.hash(),
            event.address,
            EventFlags::EnsureFlag,
            None,
            None,
            None,
            event.value,
            None,
        );
        LEAF.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(T1_P1_B_WRITE_LEAF_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_t1_p1_b_write_leaf() -> Result<ProofWithPublicInputs<F, C, D>> {
        let event = EVENT_T1_P1_B_WRITE;
        let proof = LEAF.prove(&BRANCH, event.address, event.owner, event.ty, event.value)?;
        assert_proof(
            &proof,
            event.hash(),
            event.address,
            EventFlags::WriteFlag,
            event.owner,
            None,
            None,
            event.value,
            None,
        );
        LEAF.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(T1_P1_B_GIVE_LEAF_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_t1_p1_b_give_leaf() -> Result<ProofWithPublicInputs<F, C, D>> {
        let event = EVENT_T1_P1_B_GIVE;
        let proof = LEAF.prove(&BRANCH, event.address, event.owner, event.ty, event.value)?;
        assert_proof(
            &proof,
            event.hash(),
            event.address,
            EventFlags::GiveOwnerFlag,
            event.owner,
            event.value,
            None,
            None,
            None,
        );
        LEAF.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(T1_P1_B_CREDIT_LEAF_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_t1_p1_b_credit_leaf() -> Result<ProofWithPublicInputs<F, C, D>> {
        let event = EVENT_T1_P1_B_CREDIT;
        let proof = LEAF.prove(&BRANCH, event.address, event.owner, event.ty, event.value)?;
        assert_proof(
            &proof,
            event.hash(),
            event.address,
            BitFlags::empty(),
            event.owner,
            None,
            None,
            None,
            -event.value[0],
        );
        LEAF.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(T1_P2_A_READ_LEAF_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_t1_p2_a_read_leaf() -> Result<ProofWithPublicInputs<F, C, D>> {
        let event = EVENT_T1_P2_A_READ;
        let proof = LEAF.prove(&BRANCH, event.address, event.owner, event.ty, event.value)?;
        assert_proof(
            &proof,
            event.hash(),
            event.address,
            EventFlags::ReadFlag,
            None,
            None,
            event.value,
            None,
            None,
        );
        LEAF.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(T1_P2_D_READ_LEAF_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_t1_p2_d_read_leaf() -> Result<ProofWithPublicInputs<F, C, D>> {
        let event = EVENT_T1_P2_D_READ;
        let proof = LEAF.prove(&BRANCH, event.address, event.owner, event.ty, event.value)?;
        assert_proof(
            &proof,
            event.hash(),
            event.address,
            EventFlags::ReadFlag,
            None,
            None,
            event.value,
            None,
            None,
        );
        LEAF.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(pub A_BRANCH_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_a_branch() -> Result<ProofWithPublicInputs<F, C, D>> {
        let address = ADDRESS_A;
        let old = &STATE_0[address];
        let new = &STATE_1[address];
        let address = address as u64;
        let credit_delta = -EVENT_T0_P0_A_CREDIT.value[0];

        let p0_proof = BRANCH.prove(
            true,
            &T0_P0_A_WRITE_LEAF_PROOF,
            Some((true, &T0_P0_A_CREDIT_LEAF_PROOF)),
        )?;
        assert_proof(
            &p0_proof,
            *T0_P0_HASH,
            address,
            EventFlags::WriteFlag,
            old.constraint_owner,
            None,
            None,
            new.data,
            credit_delta,
        );
        BRANCH.circuit.verify(p0_proof.clone())?;

        let p2_proof = BRANCH.prove(
            true,
            &T0_P2_A_READ_LEAF_PROOF,
            Some((true, &T0_P2_A_ENSURE_LEAF_PROOF)),
        )?;
        assert_proof(
            &p2_proof,
            *T0_P2_A_HASH,
            address,
            EventFlags::ReadFlag | EventFlags::EnsureFlag,
            None,
            None,
            old.data,
            new.data,
            None,
        );
        BRANCH.circuit.verify(p2_proof.clone())?;

        let t0_proof = BRANCH.prove(false, &p0_proof, Some((false, &p2_proof)))?;
        assert_proof(
            &t0_proof,
            *T0_A_HASH,
            address,
            EventFlags::WriteFlag | EventFlags::ReadFlag | EventFlags::EnsureFlag,
            old.constraint_owner,
            None,
            old.data,
            new.data,
            credit_delta,
        );
        BRANCH.circuit.verify(t0_proof.clone())?;

        let root_proof = BRANCH.prove(false, &t0_proof, Some((true, &T1_P2_A_READ_LEAF_PROOF)))?;
        assert_proof(
            &root_proof,
            *T0_T1_A_HASH,
            address,
            EventFlags::WriteFlag | EventFlags::ReadFlag | EventFlags::EnsureFlag,
            old.constraint_owner,
            None,
            old.data,
            new.data,
            credit_delta,
        );
        BRANCH.circuit.verify(root_proof.clone())?;

        Ok(root_proof)
    }

    #[tested_fixture::tested_fixture(pub B_BRANCH_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_b_branch() -> Result<ProofWithPublicInputs<F, C, D>> {
        let address = ADDRESS_B;
        let old = &STATE_0[address];
        let new = &STATE_1[address];
        let address = address as u64;
        let credit_delta = -EVENT_T1_P1_B_CREDIT.value[0];

        let pm_proof = BRANCH.prove(
            true,
            &T1_PM_B_TAKE_LEAF_PROOF,
            Some((true, &T1_PM_B_ENSURE_LEAF_PROOF)),
        )?;
        assert_proof(
            &pm_proof,
            *T1_PM_HASH,
            address,
            EventFlags::TakeOwnerFlag | EventFlags::EnsureFlag,
            old.constraint_owner,
            new.constraint_owner,
            None,
            new.data,
            None,
        );
        BRANCH.circuit.verify(pm_proof.clone())?;

        let p1_proof_1 = BRANCH.prove(
            true,
            &T1_P1_B_WRITE_LEAF_PROOF,
            Some((true, &T1_P1_B_GIVE_LEAF_PROOF)),
        )?;
        assert_proof(
            &p1_proof_1,
            *T1_P1_B_WRITE_GIVE_HASH,
            address,
            EventFlags::GiveOwnerFlag | EventFlags::WriteFlag,
            old.constraint_owner,
            new.constraint_owner,
            None,
            new.data,
            None,
        );
        BRANCH.circuit.verify(p1_proof_1.clone())?;

        let p1_proof_2 =
            BRANCH.prove(false, &p1_proof_1, Some((true, &T1_P1_B_CREDIT_LEAF_PROOF)))?;
        assert_proof(
            &p1_proof_2,
            *T1_P1_HASH,
            address,
            EventFlags::GiveOwnerFlag | EventFlags::WriteFlag,
            old.constraint_owner,
            new.constraint_owner,
            None,
            new.data,
            credit_delta,
        );
        BRANCH.circuit.verify(p1_proof_2.clone())?;

        let root_proof = BRANCH.prove(false, &pm_proof, Some((false, &p1_proof_2)))?;
        assert_proof(
            &root_proof,
            *T1_B_HASH,
            address,
            EventFlags::TakeOwnerFlag
                | EventFlags::EnsureFlag
                | EventFlags::GiveOwnerFlag
                | EventFlags::WriteFlag,
            old.constraint_owner,
            new.constraint_owner,
            None,
            new.data,
            credit_delta,
        );
        BRANCH.circuit.verify(root_proof.clone())?;

        Ok(root_proof)
    }

    #[tested_fixture::tested_fixture(pub C_BRANCH_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_c_branch() -> Result<ProofWithPublicInputs<F, C, D>> {
        let address = ADDRESS_C;
        let old = &STATE_0[address];
        let new = &STATE_1[address];
        let address = address as u64;
        let credit_delta = EVENT_T0_PM_C_CREDIT.value[0];

        let pm_proof_1 = BRANCH.prove(
            true,
            &T0_PM_C_CREDIT_LEAF_PROOF,
            Some((true, &T0_PM_C_GIVE_LEAF_PROOF)),
        )?;
        assert_proof(
            &pm_proof_1,
            *T0_PM_C_CREDIT_GIVE_HASH,
            address,
            EventFlags::GiveOwnerFlag,
            old.constraint_owner,
            new.constraint_owner,
            None,
            None,
            credit_delta,
        );
        BRANCH.circuit.verify(pm_proof_1.clone())?;

        let pm_proof_2 =
            BRANCH.prove(false, &pm_proof_1, Some((true, &T0_PM_C_WRITE_LEAF_PROOF)))?;
        assert_proof(
            &pm_proof_2,
            *T0_PM_HASH,
            address,
            EventFlags::GiveOwnerFlag | EventFlags::WriteFlag,
            old.constraint_owner,
            new.constraint_owner,
            None,
            new.data,
            credit_delta,
        );
        BRANCH.circuit.verify(pm_proof_2.clone())?;

        let root_proof =
            BRANCH.prove(false, &pm_proof_2, Some((true, &T0_P2_C_TAKE_LEAF_PROOF)))?;
        assert_proof(
            &root_proof,
            *T0_C_HASH,
            address,
            EventFlags::GiveOwnerFlag | EventFlags::WriteFlag | EventFlags::TakeOwnerFlag,
            old.constraint_owner,
            new.constraint_owner,
            None,
            new.data,
            credit_delta,
        );
        BRANCH.circuit.verify(root_proof.clone())?;

        Ok(root_proof)
    }

    #[tested_fixture::tested_fixture(pub D_BRANCH_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_d_branch() -> Result<ProofWithPublicInputs<F, C, D>> {
        let address = ADDRESS_D;
        let old = &STATE_0[address];
        let address = address as u64;

        let proof = BRANCH.prove(true, &T1_P2_D_READ_LEAF_PROOF, None)?;
        assert_proof(
            &proof,
            *T1_P2_D_HASH,
            address,
            EventFlags::ReadFlag,
            None,
            None,
            old.data,
            None,
            None,
        );
        BRANCH.circuit.verify(proof.clone())?;

        Ok(proof)
    }
}

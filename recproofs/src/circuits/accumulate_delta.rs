//! Circuits for proving events can be accumulated to a state delta object.

use std::marker::PhantomData;

use anyhow::Result;
use enumflags2::BitFlags;
use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::{HashOut, RichField};
use plonky2::iop::witness::PartialWitness;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::{CircuitConfig, CircuitData, VerifierOnlyCircuitData};
use plonky2::plonk::config::{AlgebraicHasher, GenericConfig, Hasher};

use super::{Branch, IsLeaf, Leaf};
use crate::subcircuits::unpruned::PartialAllowed;
use crate::subcircuits::{unbounded, unpruned};
use crate::{hash_event, Event, EventFlags, EventType};

// The core subcircuit for this circuit
pub mod core;

#[derive(Clone)]
pub struct Indices {
    pub unbounded: unbounded::PublicIndices,
    pub event_hash: unpruned::PublicIndices,
    pub partial_state: core::PublicIndices,
}

pub type Proof<T, F, C, const D: usize> = super::Proof<T, Indices, F, C, D>;

pub type LeafProof<F, C, const D: usize> = Proof<Leaf, F, C, D>;

pub type BranchProof<F, C, const D: usize> = Proof<Branch, F, C, D>;

impl<T, F, C, const D: usize> Proof<T, F, C, D>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    C::Hasher: Hasher<F, Hash = HashOut<F>>,
{
    pub fn verifier(&self) -> VerifierOnlyCircuitData<C, D> {
        self.indices
            .unbounded
            .verifier
            .get_field(&self.proof.public_inputs)
    }

    pub fn events(&self) -> HashOut<F> {
        self.indices
            .event_hash
            .unpruned_hash
            .get_field(&self.proof.public_inputs)
    }

    pub fn address(&self) -> u64 {
        self.indices
            .partial_state
            .address
            .get_field(&self.proof.public_inputs)
            .to_canonical_u64()
    }

    pub fn object_flags(&self) -> BitFlags<EventFlags> {
        let flags = self
            .indices
            .partial_state
            .object_flags
            .get_field(&self.proof.public_inputs)
            .to_canonical_u64();
        BitFlags::from_bits(flags.try_into().unwrap()).unwrap()
    }

    pub fn old_owner(&self) -> [F; 4] {
        self.indices
            .partial_state
            .old_owner
            .get_field(&self.proof.public_inputs)
    }

    pub fn new_owner(&self) -> [F; 4] {
        self.indices
            .partial_state
            .new_owner
            .get_field(&self.proof.public_inputs)
    }

    pub fn old_data(&self) -> [F; 4] {
        self.indices
            .partial_state
            .old_data
            .get_field(&self.proof.public_inputs)
    }

    pub fn new_data(&self) -> [F; 4] {
        self.indices
            .partial_state
            .new_data
            .get_field(&self.proof.public_inputs)
    }

    pub fn credit_delta(&self) -> F {
        self.indices
            .partial_state
            .credit_delta
            .get_field(&self.proof.public_inputs)
    }
}

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

    fn indices(&self) -> Indices {
        Indices {
            unbounded: self.unbounded.indices.clone(),
            event_hash: self.event_hash.indices,
            partial_state: self.partial_state.indices,
        }
    }

    pub fn prove(
        &self,
        branch: &BranchCircuit<F, C, D>,
        address: u64,
        event_owner: [F; 4],
        event_ty: EventType,
        event_value: [F; 4],
    ) -> Result<LeafProof<F, C, D>> {
        let mut inputs = PartialWitness::new();
        self.unbounded.set_witness(&mut inputs, &branch.circuit);
        self.partial_state.set_witness(&mut inputs, Event {
            owner: event_owner,
            ty: event_ty,
            address,
            value: event_value,
        });
        let proof = self.circuit.prove(inputs)?;
        Ok(LeafProof {
            proof,
            tag: PhantomData,
            indices: self.indices(),
        })
    }

    pub fn verify(&self, proof: LeafProof<F, C, D>) -> Result<()> {
        self.circuit.verify(proof.proof)
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

    fn indices(&self) -> Indices {
        Indices {
            unbounded: self.unbounded.indices.clone(),
            event_hash: self.event_hash.indices,
            partial_state: self.partial_state.indices,
        }
    }

    pub fn prove<L: IsLeaf, R: IsLeaf>(
        &self,
        left_proof: &Proof<L, F, C, D>,
        right_proof: Option<&Proof<R, F, C, D>>,
    ) -> Result<BranchProof<F, C, D>> {
        let mut inputs = PartialWitness::new();
        let partial = right_proof.is_none();
        let (right_is_leaf, right_proof) = if let Some(right_proof) = right_proof {
            (R::VALUE, &right_proof.proof)
        } else {
            (L::VALUE, &left_proof.proof)
        };
        self.unbounded.set_witness(
            &mut inputs,
            L::VALUE,
            &left_proof.proof,
            right_is_leaf,
            right_proof,
        );
        self.event_hash.set_witness(&mut inputs, partial);
        if partial {
            self.partial_state
                .set_witness_from_proof(&mut inputs, &left_proof.proof.public_inputs);
        } else {
            self.partial_state.set_witness_from_proofs(
                &mut inputs,
                &left_proof.proof.public_inputs,
                &right_proof.public_inputs,
            );
        }
        let proof = self.circuit.prove(inputs)?;
        Ok(BranchProof {
            proof,
            tag: PhantomData,
            indices: self.indices(),
        })
    }

    pub fn verify(&self, proof: BranchProof<F, C, D>) -> Result<()> {
        self.circuit.verify(proof.proof)
    }
}

#[cfg(test)]
pub mod test {
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

    #[tested_fixture::tested_fixture(pub LEAF)]
    fn build_leaf() -> LeafCircuit<F, C, D> { LeafCircuit::new(&CONFIG) }

    #[tested_fixture::tested_fixture(pub BRANCH)]
    fn build_branch() -> BranchCircuit<F, C, D> { BranchCircuit::new(&CONFIG, &LEAF) }

    #[allow(clippy::too_many_arguments)]
    fn assert_proof<T>(
        proof: &Proof<T, F, C, D>,
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

        let p_address = proof.address();
        assert_eq!(p_address, address);
        let p_flags = proof.object_flags();
        assert_eq!(p_flags, flags.into());
        let p_old_owner = proof.old_owner();
        assert_eq!(p_old_owner, old_owner.into().unwrap_or_default());
        let p_new_owner = proof.new_owner();
        assert_eq!(p_new_owner, new_owner.into().unwrap_or_default());
        let p_old_data = proof.old_data();
        assert_eq!(p_old_data, old_data.into().unwrap_or_default());
        let p_new_data = proof.new_data();
        assert_eq!(p_new_data, new_data.into().unwrap_or_default());
        let p_credit_delta = proof.credit_delta();
        assert_eq!(p_credit_delta, credit_delta.into().unwrap_or_default());

        let indices = &LEAF.event_hash.indices;
        assert_eq!(*indices, BRANCH.event_hash.indices);

        let p_event_hash = proof.events();
        assert_eq!(p_event_hash, event_hash);
    }

    #[tested_fixture::tested_fixture(T0_PM_C_CREDIT_LEAF_PROOF: LeafProof<F, C, D>)]
    fn verify_t0_pm_c_credit_leaf() -> Result<LeafProof<F, C, D>> {
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
        LEAF.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(T0_PM_C_GIVE_LEAF_PROOF: LeafProof<F, C, D>)]
    fn verify_t0_pm_c_give_leaf() -> Result<LeafProof<F, C, D>> {
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
        LEAF.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(T0_PM_C_WRITE_LEAF_PROOF: LeafProof<F, C, D>)]
    fn verify_t0_pm_c_write_leaf() -> Result<LeafProof<F, C, D>> {
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
        LEAF.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(T0_P0_A_WRITE_LEAF_PROOF: LeafProof<F, C, D>)]
    fn verify_t0_p0_a_write_leaf() -> Result<LeafProof<F, C, D>> {
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
        LEAF.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(T0_P0_A_CREDIT_LEAF_PROOF: LeafProof<F, C, D>)]
    fn verify_t0_p0_a_credit_leaf() -> Result<LeafProof<F, C, D>> {
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
        LEAF.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(T0_P2_A_READ_LEAF_PROOF: LeafProof<F, C, D>)]
    fn verify_t0_p2_a_read_leaf() -> Result<LeafProof<F, C, D>> {
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
        LEAF.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(T0_P2_A_ENSURE_LEAF_PROOF: LeafProof<F, C, D>)]
    fn verify_t0_p2_a_ensure_leaf() -> Result<LeafProof<F, C, D>> {
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
        LEAF.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(T0_P2_C_TAKE_LEAF_PROOF: LeafProof<F, C, D>)]
    fn verify_t0_p2_c_take_leaf() -> Result<LeafProof<F, C, D>> {
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
        LEAF.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(T1_PM_B_TAKE_LEAF_PROOF: LeafProof<F, C, D>)]
    fn verify_t1_pm_b_take_leaf() -> Result<LeafProof<F, C, D>> {
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
        LEAF.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(T1_PM_B_ENSURE_LEAF_PROOF: LeafProof<F, C, D>)]
    fn verify_t1_pm_b_ensure_leaf() -> Result<LeafProof<F, C, D>> {
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
        LEAF.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(T1_P1_B_WRITE_LEAF_PROOF: LeafProof<F, C, D>)]
    fn verify_t1_p1_b_write_leaf() -> Result<LeafProof<F, C, D>> {
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
        LEAF.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(T1_P1_B_GIVE_LEAF_PROOF: LeafProof<F, C, D>)]
    fn verify_t1_p1_b_give_leaf() -> Result<LeafProof<F, C, D>> {
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
        LEAF.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(T1_P1_B_CREDIT_LEAF_PROOF: LeafProof<F, C, D>)]
    fn verify_t1_p1_b_credit_leaf() -> Result<LeafProof<F, C, D>> {
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
        LEAF.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(T1_P2_A_READ_LEAF_PROOF: LeafProof<F, C, D>)]
    fn verify_t1_p2_a_read_leaf() -> Result<LeafProof<F, C, D>> {
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
        LEAF.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(T1_P2_D_READ_LEAF_PROOF: LeafProof<F, C, D>)]
    fn verify_t1_p2_d_read_leaf() -> Result<LeafProof<F, C, D>> {
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
        LEAF.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(pub A_BRANCH_PROOF: BranchProof<F, C, D>)]
    fn verify_a_branch() -> Result<BranchProof<F, C, D>> {
        let address = ADDRESS_A;
        let old = &STATE_0[address];
        let new = &STATE_1[address];
        let address = address as u64;
        let credit_delta = -EVENT_T0_P0_A_CREDIT.value[0];

        let p0_proof = BRANCH.prove(&T0_P0_A_WRITE_LEAF_PROOF, Some(&T0_P0_A_CREDIT_LEAF_PROOF))?;
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
        BRANCH.verify(p0_proof.clone())?;

        let p2_proof = BRANCH.prove(&T0_P2_A_READ_LEAF_PROOF, Some(&T0_P2_A_ENSURE_LEAF_PROOF))?;
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
        BRANCH.verify(p2_proof.clone())?;

        let t0_proof = BRANCH.prove(&p0_proof, Some(&p2_proof))?;
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
        BRANCH.verify(t0_proof.clone())?;

        let root_proof = BRANCH.prove(&t0_proof, Some(&T1_P2_A_READ_LEAF_PROOF))?;
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
        BRANCH.verify(root_proof.clone())?;

        Ok(root_proof)
    }

    #[tested_fixture::tested_fixture(pub B_BRANCH_PROOF: BranchProof<F, C, D>)]
    fn verify_b_branch() -> Result<BranchProof<F, C, D>> {
        let address = ADDRESS_B;
        let old = &STATE_0[address];
        let new = &STATE_1[address];
        let address = address as u64;
        let credit_delta = -EVENT_T1_P1_B_CREDIT.value[0];

        let pm_proof = BRANCH.prove(&T1_PM_B_TAKE_LEAF_PROOF, Some(&T1_PM_B_ENSURE_LEAF_PROOF))?;
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
        BRANCH.verify(pm_proof.clone())?;

        let p1_proof_1 = BRANCH.prove(&T1_P1_B_WRITE_LEAF_PROOF, Some(&T1_P1_B_GIVE_LEAF_PROOF))?;
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
        BRANCH.verify(p1_proof_1.clone())?;

        let p1_proof_2 = BRANCH.prove(&p1_proof_1, Some(&T1_P1_B_CREDIT_LEAF_PROOF))?;
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
        BRANCH.verify(p1_proof_2.clone())?;

        let root_proof = BRANCH.prove(&pm_proof, Some(&p1_proof_2))?;
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
        BRANCH.verify(root_proof.clone())?;

        Ok(root_proof)
    }

    #[tested_fixture::tested_fixture(pub C_BRANCH_PROOF: BranchProof<F, C, D>)]
    fn verify_c_branch() -> Result<BranchProof<F, C, D>> {
        let address = ADDRESS_C;
        let old = &STATE_0[address];
        let new = &STATE_1[address];
        let address = address as u64;
        let credit_delta = EVENT_T0_PM_C_CREDIT.value[0];

        let pm_proof_1 =
            BRANCH.prove(&T0_PM_C_CREDIT_LEAF_PROOF, Some(&T0_PM_C_GIVE_LEAF_PROOF))?;
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
        BRANCH.verify(pm_proof_1.clone())?;

        let pm_proof_2 = BRANCH.prove(&pm_proof_1, Some(&T0_PM_C_WRITE_LEAF_PROOF))?;
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
        BRANCH.verify(pm_proof_2.clone())?;

        let root_proof = BRANCH.prove(&pm_proof_2, Some(&T0_P2_C_TAKE_LEAF_PROOF))?;
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
        BRANCH.verify(root_proof.clone())?;

        Ok(root_proof)
    }

    #[tested_fixture::tested_fixture(pub D_BRANCH_PROOF: BranchProof<F, C, D>)]
    fn verify_d_branch() -> Result<BranchProof<F, C, D>> {
        let address = ADDRESS_D;
        let old = &STATE_0[address];
        let address = address as u64;

        let proof = BRANCH.prove(&T1_P2_D_READ_LEAF_PROOF, None::<&LeafProof<_, _, D>>)?;
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
        BRANCH.verify(proof.clone())?;

        Ok(proof)
    }
}

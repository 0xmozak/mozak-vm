//! Circuits for proving events can be summarized to a commitment.

use std::marker::PhantomData;

use anyhow::Result;
use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::{HashOut, RichField};
use plonky2::iop::target::Target;
use plonky2::iop::witness::{PartialWitness, WitnessWrite};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::{CircuitConfig, CircuitData, VerifierOnlyCircuitData};
use plonky2::plonk::config::{AlgebraicHasher, GenericConfig, Hasher};
use plonky2::plonk::proof::ProofWithPublicInputs;

use super::{Branch, IsLeaf, Leaf};
use crate::subcircuits::unpruned::PartialAllowed;
use crate::subcircuits::{propagate, unbounded, unpruned};
use crate::{byte_wise_hash_event, hash_event, Event};

#[derive(Clone)]
pub struct Indices {
    pub unbounded: unbounded::PublicIndices,
    pub hash: unpruned::PublicIndices,
    pub vm_hash: unpruned::PublicIndices,
    pub event_owner: propagate::PublicIndices<4>,
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

    pub fn hash(&self) -> HashOut<F> {
        self.indices
            .hash
            .unpruned_hash
            .get_field(&self.proof.public_inputs)
    }

    pub fn vm_hash(&self) -> HashOut<F> {
        self.indices
            .vm_hash
            .unpruned_hash
            .get_field(&self.proof.public_inputs)
    }

    pub fn event_owner(&self) -> [F; 4] {
        self.indices
            .event_owner
            .values
            .get_field(&self.proof.public_inputs)
    }
}

pub struct LeafTargets {
    /// The event type
    pub event_ty: Target,

    /// The event address
    pub event_address: Target,

    /// The event value
    pub event_value: [Target; 4],
}

pub struct LeafCircuit<F, C, const D: usize>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>, {
    /// The recursion subcircuit
    pub unbounded: unbounded::LeafSubCircuit,

    /// The rp-style merkle hash of all event fields
    pub hash: unpruned::LeafSubCircuit,

    /// The vm-style merkle hash of all event fields
    pub vm_hash: unpruned::LeafSubCircuit,

    /// The owner of this event propagated throughout this tree
    pub event_owner: propagate::LeafSubCircuit<4>,

    /// The other event fields
    pub targets: LeafTargets,

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
        let hash_inputs = unpruned::SubCircuitInputs::default(&mut builder);
        let vm_hash_inputs = unpruned::SubCircuitInputs::default(&mut builder);
        let event_owner_inputs = propagate::SubCircuitInputs::<4>::default(&mut builder);

        let unbounded_targets = unbounded_inputs.build_leaf::<F, C, D>(&mut builder);
        let hash_targets = hash_inputs.build_leaf(&mut builder);
        let vm_hash_targets = vm_hash_inputs.build_leaf(&mut builder);
        let event_owner_targets = event_owner_inputs.build_leaf(&mut builder);

        let targets = LeafTargets {
            event_ty: builder.add_virtual_target(),
            event_address: builder.add_virtual_target(),
            event_value: builder.add_virtual_target_arr::<4>(),
        };

        let event_hash = hash_event(
            &mut builder,
            event_owner_targets.inputs.values,
            targets.event_ty,
            targets.event_address,
            targets.event_value,
        );
        let event_vm_hash = byte_wise_hash_event(
            &mut builder,
            targets.event_ty,
            targets.event_address,
            targets.event_value,
        );

        builder.connect_hashes(hash_targets.inputs.unpruned_hash, event_hash);
        builder.connect_hashes(vm_hash_targets.inputs.unpruned_hash, event_vm_hash);

        let circuit = builder.build();

        let public_inputs = &circuit.prover_only.public_inputs;
        let unbounded = unbounded_targets.build(public_inputs);
        let hash = hash_targets.build(public_inputs);
        let vm_hash = vm_hash_targets.build(public_inputs);
        let event_owner = event_owner_targets.build(public_inputs);

        Self {
            unbounded,
            hash,
            vm_hash,
            event_owner,
            targets,
            circuit,
        }
    }

    fn indices(&self) -> Indices {
        Indices {
            unbounded: self.unbounded.indices.clone(),
            hash: self.hash.indices,
            vm_hash: self.vm_hash.indices,
            event_owner: self.event_owner.indices,
        }
    }

    pub fn prove(
        &self,
        event: Event<F>,
        branch: &BranchCircuit<F, C, D>,
    ) -> Result<LeafProof<F, C, D>> {
        let mut inputs = PartialWitness::new();
        self.unbounded.set_witness(&mut inputs, &branch.circuit);
        self.event_owner.set_witness(&mut inputs, event.owner);
        inputs.set_target(self.targets.event_ty, F::from_canonical_u8(event.ty as u8));
        inputs.set_target(
            self.targets.event_address,
            F::from_canonical_u64(event.address),
        );
        inputs.set_target_arr(&self.targets.event_value, &event.value);
        let proof = self.circuit.prove(inputs)?;
        Ok(LeafProof {
            proof,
            tag: PhantomData,
            indices: self.indices(),
        })
    }

    pub fn prove_unsafe(
        &self,
        event: Event<F>,
        hash: Option<HashOut<F>>,
        vm_hash: Option<HashOut<F>>,
        branch: &BranchCircuit<F, C, D>,
    ) -> Result<LeafProof<F, C, D>> {
        let mut inputs = PartialWitness::new();
        self.unbounded.set_witness(&mut inputs, &branch.circuit);
        if let Some(hash) = hash {
            self.hash.set_witness(&mut inputs, hash);
        }
        if let Some(vm_hash) = vm_hash {
            self.vm_hash.set_witness(&mut inputs, vm_hash);
        }
        self.event_owner.set_witness(&mut inputs, event.owner);
        inputs.set_target(self.targets.event_ty, F::from_canonical_u8(event.ty as u8));
        inputs.set_target(
            self.targets.event_address,
            F::from_canonical_u64(event.address),
        );
        inputs.set_target_arr(&self.targets.event_value, &event.value);
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
    pub unbounded: unbounded::BranchSubCircuit<D>,

    /// The merkle hash of all events
    pub hash: unpruned::BranchSubCircuit<PartialAllowed>,

    /// The vm-style merkle hash of all events
    pub vm_hash: unpruned::BranchSubCircuit<PartialAllowed>,

    /// The owner of the events propagated throughout this tree
    pub event_owner: propagate::BranchSubCircuit<4>,

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
        let hash_inputs = unpruned::SubCircuitInputs::default(&mut builder);
        let vm_hash_inputs = unpruned::SubCircuitInputs::default(&mut builder);
        let event_owner_inputs = propagate::SubCircuitInputs::<4>::default(&mut builder);

        let unbounded_targets =
            unbounded_inputs.build_branch(&mut builder, &leaf.unbounded, &leaf.circuit);
        let hash_targets = hash_inputs.build_extended_branch(
            &mut builder,
            &leaf.hash.indices,
            &unbounded_targets.left_proof,
            &unbounded_targets.right_proof,
            false,
        );
        let vm_hash_targets = vm_hash_inputs.build_extended_branch(
            &mut builder,
            &leaf.vm_hash.indices,
            &unbounded_targets.left_proof,
            &unbounded_targets.right_proof,
            true,
        );
        let event_owner_targets = event_owner_inputs.build_branch(
            &mut builder,
            &leaf.event_owner.indices,
            &unbounded_targets.left_proof,
            &unbounded_targets.right_proof,
        );

        builder.connect(
            hash_targets.extension.partial.target,
            vm_hash_targets.extension.partial.target,
        );

        let circuit = builder.build();

        let public_inputs = &circuit.prover_only.public_inputs;
        let unbounded = unbounded_targets.build(&leaf.unbounded, public_inputs);
        let hash = hash_targets.build(&leaf.hash.indices, public_inputs);
        let vm_hash = vm_hash_targets.build(&leaf.vm_hash.indices, public_inputs);
        let event_owner = event_owner_targets.build(&leaf.event_owner.indices, public_inputs);

        Self {
            unbounded,
            hash,
            vm_hash,
            event_owner,
            circuit,
        }
    }

    fn indices(&self) -> Indices {
        Indices {
            unbounded: self.unbounded.indices.clone(),
            hash: self.hash.indices,
            vm_hash: self.vm_hash.indices,
            event_owner: self.event_owner.indices,
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
        self.hash.set_witness(&mut inputs, partial);
        self.vm_hash.set_witness(&mut inputs, partial);
        let proof = self.circuit.prove(inputs)?;
        Ok(BranchProof {
            proof,
            tag: PhantomData,
            indices: self.indices(),
        })
    }

    /// `hash` `vm_hash` and `event_owner` only need to be provided to check
    /// externally, otherwise they will be calculated
    #[allow(clippy::too_many_arguments)]
    pub fn prove_unsafe(
        &self,
        partial: bool,
        hash: HashOut<F>,
        vm_hash: HashOut<F>,
        event_owner: Option<[F; 4]>,
        left_is_leaf: bool,
        left_proof: &ProofWithPublicInputs<F, C, D>,
        right_is_leaf: bool,
        right_proof: &ProofWithPublicInputs<F, C, D>,
    ) -> Result<BranchProof<F, C, D>> {
        let mut inputs = PartialWitness::new();
        self.unbounded.set_witness(
            &mut inputs,
            left_is_leaf,
            left_proof,
            right_is_leaf,
            right_proof,
        );
        self.hash.set_witness_unsafe(&mut inputs, hash, partial);
        self.vm_hash
            .set_witness_unsafe(&mut inputs, vm_hash, partial);
        if let Some(event_owner) = event_owner {
            self.event_owner.set_witness(&mut inputs, event_owner);
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
    use std::panic::catch_unwind;

    use plonky2::field::types::Field;

    pub use super::BranchProof;
    use super::*;
    use crate::circuits::test_data::{
        EVENT_T0_P0_A_CREDIT, EVENT_T0_P0_A_WRITE, EVENT_T0_P2_A_ENSURE, EVENT_T0_P2_A_READ,
        EVENT_T0_P2_C_TAKE, EVENT_T0_PM_C_CREDIT, EVENT_T0_PM_C_GIVE, EVENT_T0_PM_C_WRITE,
        EVENT_T1_P1_B_CREDIT, EVENT_T1_P1_B_GIVE, EVENT_T1_P1_B_WRITE, EVENT_T1_P2_A_READ,
        EVENT_T1_P2_D_READ, EVENT_T1_PM_B_ENSURE, EVENT_T1_PM_B_TAKE,
    };
    use crate::test_utils::{hash_branch, hash_branch_bytes, C, CONFIG, D, F};
    use crate::EventType;

    #[tested_fixture::tested_fixture(pub LEAF)]
    fn build_leaf() -> LeafCircuit<F, C, D> { LeafCircuit::new(&CONFIG) }

    #[tested_fixture::tested_fixture(pub BRANCH)]
    fn build_branch() -> BranchCircuit<F, C, D> { BranchCircuit::new(&CONFIG, &LEAF) }

    fn assert_proof<T>(
        proof: &Proof<T, F, C, D>,
        hash: HashOut<F>,
        vm_hash: HashOut<F>,
        owner: [F; 4],
    ) {
        let indices = &LEAF.hash.indices;
        assert_eq!(*indices, BRANCH.hash.indices);
        let p_hash = proof.hash();
        assert_eq!(p_hash, hash);

        let indices = &LEAF.vm_hash.indices;
        assert_eq!(*indices, BRANCH.vm_hash.indices);
        let p_vm_hash = proof.vm_hash();
        assert_eq!(p_vm_hash, vm_hash);

        let p_owner = proof.event_owner();
        assert_eq!(p_owner, owner);
    }

    fn test_leaf(event: Event<F>) -> Result<LeafProof<F, C, D>> {
        let proof = LEAF.prove(event, &BRANCH)?;
        assert_proof(&proof, event.hash(), event.byte_wise_hash(), event.owner);
        LEAF.verify(proof.clone())?;
        Ok(proof)
    }

    #[allow(clippy::type_complexity)]
    fn test_branch_0(
        left: &LeafProof<F, C, D>,
        right: &LeafProof<F, C, D>,
    ) -> Result<BranchProof<F, C, D>> {
        assert_eq!(
            left.event_owner(),
            right.event_owner(),
            "Test bug: tried to combine different event owners"
        );

        let proof = BRANCH.prove(left, Some(right))?;
        let hash = hash_branch(&left.hash(), &right.hash());
        let vm_hash = hash_branch_bytes(&left.vm_hash(), &right.vm_hash());
        assert_proof(&proof, hash, vm_hash, left.event_owner());
        BRANCH.verify(proof.clone())?;

        Ok(proof)
    }

    #[allow(clippy::type_complexity)]
    fn test_branch_1(
        left: &BranchProof<F, C, D>,
        right: &LeafProof<F, C, D>,
    ) -> Result<BranchProof<F, C, D>> {
        assert_eq!(
            left.event_owner(),
            right.event_owner(),
            "Test bug: tried to combine different event owners"
        );

        let proof = BRANCH.prove(left, Some(right))?;
        let hash = hash_branch(&left.hash(), &right.hash());
        let vm_hash = hash_branch_bytes(&left.vm_hash(), &right.vm_hash());
        assert_proof(&proof, hash, vm_hash, left.event_owner());
        BRANCH.verify(proof.clone())?;

        Ok(proof)
    }

    macro_rules! make_leaf_tests {
        ($($($name:ident | $proof:ident = $event:ident),+ $(,)?)?) => {$($(
    #[tested_fixture::tested_fixture($proof: LeafProof<F, C, D>)]
    fn $name() -> Result<LeafProof<F, C, D>> {
        test_leaf($event)
    }
        )+)?};
    }

    make_leaf_tests! {
        verify_t0_pm_c_credit_leaf | T0_PM_C_CREDIT_LEAF_PROOF = EVENT_T0_PM_C_CREDIT,
        verify_t0_pm_c_give_leaf | T0_PM_C_GIVE_LEAF_PROOF = EVENT_T0_PM_C_GIVE,
        verify_t0_pm_c_write_leaf | T0_PM_C_WRITE_LEAF_PROOF = EVENT_T0_PM_C_WRITE,
        verify_t0_p0_a_write_leaf | T0_P0_A_WRITE_LEAF_PROOF = EVENT_T0_P0_A_WRITE,
        verify_t0_p0_a_credit_leaf | T0_P0_A_CREDIT_LEAF_PROOF = EVENT_T0_P0_A_CREDIT,
        verify_t0_p2_a_read_leaf | T0_P2_A_READ_LEAF_PROOF = EVENT_T0_P2_A_READ,
        verify_t0_p2_a_ensure_leaf | T0_P2_A_ENSURE_LEAF_PROOF = EVENT_T0_P2_A_ENSURE,
        verify_t0_p2_c_take_leaf | T0_P2_C_TAKE_LEAF_PROOF = EVENT_T0_P2_C_TAKE,
        verify_t1_pm_b_take_leaf | T1_PM_B_TAKE_LEAF_PROOF = EVENT_T1_PM_B_TAKE,
        verify_t1_pm_b_ensure_leaf | T1_PM_B_ENSURE_LEAF_PROOF = EVENT_T1_PM_B_ENSURE,
        verify_t1_p1_b_write_leaf | T1_P1_B_WRITE_LEAF_PROOF = EVENT_T1_P1_B_WRITE,
        verify_t1_p1_b_give_leaf | T1_P1_B_GIVE_LEAF_PROOF = EVENT_T1_P1_B_GIVE,
        verify_t1_p1_b_credit_leaf | T1_P1_B_CREDIT_LEAF_PROOF = EVENT_T1_P1_B_CREDIT,
        verify_t1_p2_a_read_leaf | T1_P2_A_READ_LEAF_PROOF = EVENT_T1_P2_A_READ,
        verify_t1_p2_d_read_leaf | T1_P2_D_READ_LEAF_PROOF = EVENT_T1_P2_D_READ,
    }

    #[tested_fixture::tested_fixture(pub T0_PM_BRANCH_PROOF: BranchProof<F, C, D>)]
    fn verify_t0_pm_branch() -> Result<BranchProof<F, C, D>> {
        let proof = test_branch_0(&T0_PM_C_CREDIT_LEAF_PROOF, &T0_PM_C_GIVE_LEAF_PROOF)?;

        test_branch_1(&proof, &T0_PM_C_WRITE_LEAF_PROOF)
    }

    #[tested_fixture::tested_fixture(pub T0_P0_BRANCH_PROOF: BranchProof<F, C, D>)]
    fn verify_t0_p0_branch() -> Result<BranchProof<F, C, D>> {
        test_branch_0(&T0_P0_A_WRITE_LEAF_PROOF, &T0_P0_A_CREDIT_LEAF_PROOF)
    }

    #[tested_fixture::tested_fixture(pub T0_P2_BRANCH_PROOF: BranchProof<F, C, D>)]
    fn verify_t0_p2_branch() -> Result<BranchProof<F, C, D>> {
        let proof = test_branch_0(&T0_P2_A_READ_LEAF_PROOF, &T0_P2_A_ENSURE_LEAF_PROOF)?;

        test_branch_1(&proof, &T0_P2_C_TAKE_LEAF_PROOF)
    }

    #[tested_fixture::tested_fixture(pub T1_PM_BRANCH_PROOF: BranchProof<F, C, D>)]
    fn verify_t1_pm_branch() -> Result<BranchProof<F, C, D>> {
        test_branch_0(&T1_PM_B_TAKE_LEAF_PROOF, &T1_PM_B_ENSURE_LEAF_PROOF)
    }

    #[tested_fixture::tested_fixture(pub T1_P1_BRANCH_PROOF: BranchProof<F, C, D>)]
    fn verify_t1_p1_branch() -> Result<BranchProof<F, C, D>> {
        let proof = test_branch_0(&T1_P1_B_WRITE_LEAF_PROOF, &T1_P1_B_GIVE_LEAF_PROOF)?;

        test_branch_1(&proof, &T1_P1_B_CREDIT_LEAF_PROOF)
    }

    #[tested_fixture::tested_fixture(pub T1_P2_BRANCH_PROOF: BranchProof<F, C, D>)]
    fn verify_t1_p2_branch() -> Result<BranchProof<F, C, D>> {
        test_branch_0(&T1_P2_A_READ_LEAF_PROOF, &T1_P2_D_READ_LEAF_PROOF)
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_leaf_hash() {
        let (read_1, read_0_hash, read_0_byte_hash) = catch_unwind(|| {
            let program_hash_1 = [4, 8, 15, 16].map(F::from_canonical_u64);
            let program_hash_2 = [2, 3, 4, 2].map(F::from_canonical_u64);

            let zero_val = [F::ZERO; 4];

            let read_0 = Event {
                address: 42,
                owner: program_hash_1,
                ty: EventType::Read,
                value: zero_val,
            };
            let read_1 = Event {
                address: 42,
                owner: program_hash_2,
                ty: EventType::Read,
                value: zero_val,
            };

            let read_0_hash = read_0.hash();
            let read_0_byte_hash = read_0.byte_wise_hash();
            (read_1, read_0_hash, read_0_byte_hash)
        })
        .expect("shouldn't fail");

        // Fail to prove with mismatched hashes
        LEAF.prove_unsafe(read_1, Some(read_0_hash), Some(read_0_byte_hash), &BRANCH)
            .unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_program_match() {
        let (program_hash_1, branch_1_hash, branch_1_bytes_hash, read_proof_1, read_proof_2) =
            catch_unwind(|| {
                let program_hash_1 = [4, 8, 15, 16].map(F::from_canonical_u64);
                let program_hash_2 = [2, 3, 4, 2].map(F::from_canonical_u64);

                let zero_val = [F::ZERO; 4];

                // Read events from two different programs
                let read_0 = Event {
                    address: 42,
                    owner: program_hash_1,
                    ty: EventType::Read,
                    value: zero_val,
                };
                let read_1 = Event {
                    address: 42,
                    owner: program_hash_2,
                    ty: EventType::Read,
                    value: zero_val,
                };

                let read_0_hash = read_0.hash();
                let read_1_hash = read_1.hash();
                let read_0_byte_hash = read_0.byte_wise_hash();
                let read_1_byte_hash = read_1.byte_wise_hash();

                // Read zero
                let read_proof_1 = LEAF.prove(read_0, &BRANCH).unwrap();
                LEAF.verify(read_proof_1.clone()).unwrap();

                let read_proof_2 = LEAF.prove(read_1, &BRANCH).unwrap();
                LEAF.verify(read_proof_2.clone()).unwrap();

                // Combine reads
                let branch_1_hash = hash_branch(&read_0_hash, &read_1_hash);
                let branch_1_bytes_hash = hash_branch_bytes(&read_0_byte_hash, &read_1_byte_hash);
                (
                    program_hash_1,
                    branch_1_hash,
                    branch_1_bytes_hash,
                    read_proof_1,
                    read_proof_2,
                )
            })
            .expect("shouldn't fail");

        // Fail to prove with mismatched program hashes between branches
        // This tree requires all events are from the same program
        BRANCH
            .prove_unsafe(
                true,
                branch_1_hash,
                branch_1_bytes_hash,
                Some(program_hash_1),
                true,
                &read_proof_1.proof,
                true,
                &read_proof_2.proof,
            )
            .unwrap();
    }
}

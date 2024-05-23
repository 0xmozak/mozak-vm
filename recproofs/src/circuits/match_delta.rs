//! Circuits for recursively proving state delta objects match summarized state
//! updates

use std::marker::PhantomData;

use anyhow::Result;
use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::{HashOut, RichField};
use plonky2::iop::witness::{PartialWitness, WitnessWrite};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::{CircuitConfig, CircuitData, VerifierOnlyCircuitData};
use plonky2::plonk::config::{AlgebraicHasher, GenericConfig, Hasher};
use plonky2::plonk::proof::ProofWithPublicInputsTarget;

use super::{accumulate_delta, Branch, Leaf};
use crate::circuits::accumulate_delta::core::SplitFlags;
use crate::subcircuits::unpruned::{self, PartialAllowed};
use crate::subcircuits::{propagate, unbounded};
use crate::{connect_arrays, maybe_connect};

// The core subcircuit for this circuit
pub mod core;

#[derive(Clone)]
pub struct Indices {
    pub unbounded: unbounded::PublicIndices,
    pub event_hash: unpruned::PublicIndices,
    pub state_hash: unpruned::PublicIndices,
    pub compare_delta: core::PublicIndices,
    pub block_height: propagate::PublicIndices<1>,
}

pub type Proof<T, F, C, const D: usize> = super::Proof<T, Indices, F, C, D>;

pub type LeafProof<F, C, const D: usize> = Proof<Leaf, F, C, D>;

pub type BranchProof<F, C, const D: usize> = Proof<Branch, F, C, D>;

pub type LeafOrBranchRef<'a, F, C, const D: usize> = super::LeafOrBranchRef<'a, Indices, F, C, D>;

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

    pub fn state_hash(&self) -> HashOut<F> {
        self.indices
            .state_hash
            .unpruned_hash
            .get_field(&self.proof.public_inputs)
    }

    pub fn block_height(&self) -> u64 {
        self.indices
            .block_height
            .values
            .get_field(&self.proof.public_inputs)[0]
            .to_canonical_u64()
    }
}

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

    /// The block height
    pub block_height: propagate::LeafSubCircuit<1>,

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
        let block_height_inputs = propagate::SubCircuitInputs::default(&mut builder);

        let unbounded_targets = unbounded_inputs.build_leaf::<F, C, D>(&mut builder);
        let event_hash_targets = event_hash_inputs.build_leaf(&mut builder);
        let state_hash_targets = state_hash_inputs.build_leaf(&mut builder);
        let compare_delta_targets = compare_delta_inputs.build_leaf(&mut builder);
        let block_height_targets = block_height_inputs.build_leaf(&mut builder);

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
                .get_target(&targets.accumulate_event.public_inputs);
            builder.connect_hashes(event_hash_targets.inputs.unpruned_hash, acc_event_hash);
        }
        // address
        {
            let acc_addr = accumulate_event_circuit
                .partial_state
                .indices
                .address
                .get_target(&targets.accumulate_event.public_inputs);
            builder.connect(compare_delta_targets.address, acc_addr);
        }
        // flags
        let acc_flags = accumulate_event_circuit
            .partial_state
            .indices
            .object_flags
            .get_target(&targets.accumulate_event.public_inputs);
        builder.connect(compare_delta_targets.object_flags, acc_flags);
        let acc_flags = SplitFlags::split(&mut builder, acc_flags);
        let has_owner = builder.is_nonzero(acc_flags.owner);
        // old owner
        {
            let acc_old_owner = accumulate_event_circuit
                .partial_state
                .indices
                .old_owner
                .get_target(&targets.accumulate_event.public_inputs);
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
                .get_target(&targets.accumulate_event.public_inputs);
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
                .get_target(&targets.accumulate_event.public_inputs);
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
                .get_target(&targets.accumulate_event.public_inputs);
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
                .get_target(&targets.accumulate_event.public_inputs);
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

        // Connect the block height
        connect_arrays(&mut builder, block_height_targets.inputs.values, [
            compare_delta_targets.block_height,
        ]);

        let circuit = builder.build();

        let public_inputs = &circuit.prover_only.public_inputs;
        let unbounded = unbounded_targets.build(public_inputs);
        let event_hash = event_hash_targets.build(public_inputs);
        let state_hash = state_hash_targets.build(public_inputs);
        let compare_delta = compare_delta_targets.build(public_inputs);
        let block_height = block_height_targets.build(public_inputs);

        Self {
            unbounded,
            event_hash,
            state_hash,
            compare_delta,
            block_height,
            targets,
            circuit,
        }
    }

    fn indices(&self) -> Indices {
        Indices {
            unbounded: self.unbounded.indices.clone(),
            event_hash: self.event_hash.indices,
            state_hash: self.state_hash.indices,
            compare_delta: self.compare_delta.indices,
            block_height: self.block_height.indices,
        }
    }

    pub fn prove(
        &self,
        branch: &BranchCircuit<F, C, D>,
        accumulate_event_proof: &accumulate_delta::BranchProof<F, C, D>,
        v: LeafWitnessValue<F>,
    ) -> Result<LeafProof<F, C, D>> {
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
        inputs.set_proof_with_pis_target(
            &self.targets.accumulate_event,
            &accumulate_event_proof.proof,
        );
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

    /// The rp-style merkle hash of events
    pub event_hash: unpruned::BranchSubCircuit<PartialAllowed>,

    /// The rp-style merkle hash of summarized state updates
    pub state_hash: unpruned::BranchSubCircuit<PartialAllowed>,

    /// The partial-object/state update comparator
    pub compare_delta: core::BranchSubCircuit,

    /// The block height
    pub block_height: propagate::BranchSubCircuit<1>,

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
        let block_height_inputs = propagate::SubCircuitInputs::default(&mut builder);

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
        let block_height_targets = block_height_inputs.build_branch(
            &mut builder,
            &leaf.block_height.indices,
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
        let block_height = block_height_targets.build(&leaf.block_height.indices, public_inputs);

        Self {
            unbounded,
            event_hash,
            state_hash,
            compare_delta,
            block_height,
            circuit,
        }
    }

    fn indices(&self) -> Indices {
        Indices {
            unbounded: self.unbounded.indices.clone(),
            event_hash: self.event_hash.indices,
            state_hash: self.state_hash.indices,
            compare_delta: self.compare_delta.indices,
            block_height: self.block_height.indices,
        }
    }

    fn prove_helper(
        &self,
        left_proof: LeafOrBranchRef<'_, F, C, D>,
        right_proof: LeafOrBranchRef<'_, F, C, D>,
        partial: bool,
    ) -> Result<BranchProof<F, C, D>> {
        let mut inputs = PartialWitness::new();
        self.unbounded.set_witness(
            &mut inputs,
            left_proof.is_leaf(),
            left_proof.proof(),
            right_proof.is_leaf(),
            right_proof.proof(),
        );
        self.event_hash.set_witness(&mut inputs, partial);
        self.state_hash.set_witness(&mut inputs, partial);
        let proof = self.circuit.prove(inputs)?;
        Ok(BranchProof {
            proof,
            tag: PhantomData,
            indices: self.indices(),
        })
    }

    pub fn prove<'a>(
        &self,
        left_proof: impl Into<LeafOrBranchRef<'a, F, C, D>>,
        right_proof: impl Into<LeafOrBranchRef<'a, F, C, D>>,
    ) -> Result<BranchProof<F, C, D>>
    where
        C: 'a, {
        self.prove_helper(left_proof.into(), right_proof.into(), false)
    }

    pub fn prove_one<'a>(
        &self,
        left_proof: impl Into<LeafOrBranchRef<'a, F, C, D>>,
    ) -> Result<BranchProof<F, C, D>>
    where
        C: 'a, {
        let left_proof = left_proof.into();
        self.prove_helper(left_proof, left_proof, true)
    }

    pub fn verify(&self, proof: BranchProof<F, C, D>) -> Result<()> {
        self.circuit.verify(proof.proof)
    }
}

#[cfg(test)]
pub mod test {
    use plonky2::field::types::PrimeField64;

    use super::*;
    use crate::circuits::accumulate_delta::test as acc;
    use crate::circuits::test_data::{
        ADDRESS_A, ADDRESS_A_SUMMARY_HASH, ADDRESS_B, ADDRESS_BCD_SUMMARY_HASH,
        ADDRESS_BC_SUMMARY_HASH, ADDRESS_B_SUMMARY_HASH, ADDRESS_C, ADDRESS_C_SUMMARY_HASH,
        ADDRESS_D, ADDRESS_D_SUMMARY_HASH, ROOT_SUMMARY_HASH, STATE_0, STATE_1, T0_C_HASH,
        T0_T1_A_HASH, T0_T1_BCD_HASH, T0_T1_BC_HASH, T0_T1_HASH, T1_B_HASH, T1_P2_D_HASH,
    };
    use crate::test_utils::{C, CONFIG, D, F};

    #[tested_fixture::tested_fixture(pub LEAF)]
    fn build_leaf() -> LeafCircuit<F, C, D> { LeafCircuit::new(&CONFIG, &acc::BRANCH) }

    #[tested_fixture::tested_fixture(pub BRANCH)]
    fn build_branch() -> BranchCircuit<F, C, D> { BranchCircuit::new(&CONFIG, &LEAF) }

    fn assert_proof<T>(
        proof: &Proof<T, F, C, D>,
        block_height: u64,
        event_hash: HashOut<F>,
        state_hash: HashOut<F>,
    ) {
        let indices = &LEAF.block_height.indices;
        assert_eq!(
            indices, &BRANCH.block_height.indices,
            "LEAF and BRANCH indicies didn't match"
        );
        let p_block_height = proof.block_height();
        assert_eq!(p_block_height, block_height, "block height didn't match");

        let indices = &LEAF.event_hash.indices;
        assert_eq!(
            indices, &BRANCH.event_hash.indices,
            "LEAF and BRANCH indicies didn't match"
        );
        let p_event_hash = proof.events();
        assert_eq!(p_event_hash, event_hash, "event hash didn't match");

        let indices = &LEAF.state_hash.indices;
        assert_eq!(
            indices, &BRANCH.state_hash.indices,
            "LEAF and BRANCH indicies didn't match"
        );
        let p_state_hash = proof.state_hash();
        assert_eq!(p_state_hash, state_hash, "state hash didn't match");
    }

    #[tested_fixture::tested_fixture(A_LEAF_PROOF: LeafProof<F, C, D>)]
    fn verify_a_leaf() -> Result<LeafProof<F, C, D>> {
        let address = ADDRESS_A;
        let (old, new) = (&STATE_0[address], &STATE_1[address]);
        let witness = LeafWitnessValue {
            block_height: 1,
            last_updated: 0,
            old_owner: old.constraint_owner,
            new_owner: new.constraint_owner,
            old_data: old.data,
            new_data: new.data,
            old_credits: old.credits.to_canonical_u64(),
            new_credits: new.credits.to_canonical_u64(),
        };
        let proof = LEAF.prove(&BRANCH, *acc::A_BRANCH_PROOF, witness)?;
        assert_proof(&proof, 1, *T0_T1_A_HASH, *ADDRESS_A_SUMMARY_HASH);
        LEAF.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(B_LEAF_PROOF: LeafProof<F, C, D>)]
    fn verify_b_leaf() -> Result<LeafProof<F, C, D>> {
        let address = ADDRESS_B;
        let (old, new) = (&STATE_0[address], &STATE_1[address]);
        let witness = LeafWitnessValue {
            block_height: 1,
            last_updated: 0,
            old_owner: old.constraint_owner,
            new_owner: new.constraint_owner,
            old_data: old.data,
            new_data: new.data,
            old_credits: old.credits.to_canonical_u64(),
            new_credits: new.credits.to_canonical_u64(),
        };
        let proof = LEAF.prove(&BRANCH, *acc::B_BRANCH_PROOF, witness)?;
        assert_proof(&proof, 1, *T1_B_HASH, *ADDRESS_B_SUMMARY_HASH);
        LEAF.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(C_LEAF_PROOF: LeafProof<F, C, D>)]
    fn verify_c_leaf() -> Result<LeafProof<F, C, D>> {
        let address = ADDRESS_C;
        let (old, new) = (&STATE_0[address], &STATE_1[address]);
        let witness = LeafWitnessValue {
            block_height: 1,
            last_updated: 0,
            old_owner: old.constraint_owner,
            new_owner: new.constraint_owner,
            old_data: old.data,
            new_data: new.data,
            old_credits: old.credits.to_canonical_u64(),
            new_credits: new.credits.to_canonical_u64(),
        };
        let proof = LEAF.prove(&BRANCH, *acc::C_BRANCH_PROOF, witness)?;
        assert_proof(&proof, 1, *T0_C_HASH, *ADDRESS_C_SUMMARY_HASH);
        LEAF.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(D_LEAF_PROOF: LeafProof<F, C, D>)]
    fn verify_d_leaf() -> Result<LeafProof<F, C, D>> {
        let address = ADDRESS_D;
        let (old, new) = (&STATE_0[address], &STATE_1[address]);
        let witness = LeafWitnessValue {
            block_height: 1,
            last_updated: 0,
            old_owner: old.constraint_owner,
            new_owner: new.constraint_owner,
            old_data: old.data,
            new_data: new.data,
            old_credits: old.credits.to_canonical_u64(),
            new_credits: new.credits.to_canonical_u64(),
        };
        let proof = LEAF.prove(&BRANCH, *acc::D_BRANCH_PROOF, witness)?;
        assert_proof(&proof, 1, *T1_P2_D_HASH, *ADDRESS_D_SUMMARY_HASH);
        LEAF.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(BC_BRANCH_PROOF: BranchProof<F, C, D>)]
    fn verify_bc_branch() -> Result<BranchProof<F, C, D>> {
        let proof = BRANCH.prove(*B_LEAF_PROOF, *C_LEAF_PROOF)?;
        assert_proof(&proof, 1, *T0_T1_BC_HASH, *ADDRESS_BC_SUMMARY_HASH);
        BRANCH.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(BCD_BRANCH_PROOF: BranchProof<F, C, D>)]
    fn verify_bcd_branch() -> Result<BranchProof<F, C, D>> {
        let proof = BRANCH.prove(*BC_BRANCH_PROOF, *D_LEAF_PROOF)?;
        assert_proof(&proof, 1, *T0_T1_BCD_HASH, *ADDRESS_BCD_SUMMARY_HASH);
        BRANCH.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(pub BRANCH_PROOF: BranchProof<F, C, D>)]
    fn verify_branch_abcd() -> Result<BranchProof<F, C, D>> {
        let proof = BRANCH.prove(*A_LEAF_PROOF, *BCD_BRANCH_PROOF)?;
        assert_proof(&proof, 1, *T0_T1_HASH, *ROOT_SUMMARY_HASH);
        BRANCH.verify(proof.clone())?;
        Ok(proof)
    }

    #[test]
    fn verify_branch_one_abcd() -> Result<()> {
        let proof = BRANCH.prove_one(*BRANCH_PROOF)?;
        assert_proof(&proof, 1, *T0_T1_HASH, *ROOT_SUMMARY_HASH);
        BRANCH.verify(proof)?;
        Ok(())
    }
}

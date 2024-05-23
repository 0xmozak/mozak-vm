//! Circuits for proving events correspond to a proof

use std::marker::PhantomData;

use anyhow::Result;
use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::{HashOut, RichField};
use plonky2::iop::witness::PartialWitness;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::{CircuitConfig, CircuitData, VerifierOnlyCircuitData};
use plonky2::plonk::config::{AlgebraicHasher, GenericConfig, Hasher};

use super::{merge, verify_program, Branch, IsLeaf, Leaf};
use crate::subcircuits::unbounded;

pub mod core;

#[derive(Clone)]
pub struct Indices {
    pub unbounded: unbounded::PublicIndices,
    pub events: merge::embed::PublicIndices,
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

    pub fn events_present(&self) -> bool {
        self.indices
            .events
            .hash_present
            .get_field(&self.proof.public_inputs)
    }

    pub fn events(&self) -> HashOut<F> {
        self.indices
            .events
            .hash
            .get_field(&self.proof.public_inputs)
    }
}

pub struct LeafCircuit<F, C, const D: usize>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>, {
    /// The recursion subcircuit
    pub unbounded: unbounded::LeafSubCircuit,

    // The events list
    pub events: merge::embed::LeafSubCircuit,

    /// The program verifier
    pub program_verifier: core::ProgramVerifierSubCircuit<D>,

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
        program: &verify_program::BranchCircuit<F, C, D>,
    ) -> Self {
        let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());

        let unbounded_inputs = unbounded::SubCircuitInputs::default(&mut builder);
        let events_inputs = merge::embed::SubCircuitInputs::default(&mut builder);

        let unbounded_targets = unbounded_inputs.build_leaf::<F, C, D>(&mut builder);
        let events_targets = events_inputs.build_leaf::<F, D>(&mut builder);

        let program_verifier_targets =
            core::ProgramSetVerifierTargets::build_targets(&mut builder, program);

        // Connect the proof to the recursion
        builder.connect_hashes(
            events_targets.inputs.hash,
            program_verifier_targets.event_root,
        );
        builder.connect(
            events_targets.inputs.hash_present.target,
            program_verifier_targets.events_present.target,
        );

        let circuit = builder.build();

        let public_inputs = &circuit.prover_only.public_inputs;
        let unbounded = unbounded_targets.build(public_inputs);
        let events = events_targets.build(public_inputs);
        let program_verifier = program_verifier_targets.build(public_inputs);

        Self {
            unbounded,
            events,
            program_verifier,
            circuit,
        }
    }

    fn indices(&self) -> Indices {
        Indices {
            unbounded: self.unbounded.indices.clone(),
            events: self.events.indices,
        }
    }

    pub fn prove(
        &self,
        branch: &BranchCircuit<F, C, D>,
        program_set_proof: &verify_program::BranchProof<F, C, D>,
    ) -> Result<LeafProof<F, C, D>> {
        let mut inputs = PartialWitness::new();
        self.unbounded.set_witness(&mut inputs, &branch.circuit);
        self.program_verifier
            .set_witness(&mut inputs, &program_set_proof.proof);
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

    // The events list
    pub events: merge::embed::BranchSubCircuit<D>,

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
        let events_inputs = merge::embed::SubCircuitInputs::default(&mut builder);

        let unbounded_targets =
            unbounded_inputs.build_branch(&mut builder, &leaf.unbounded, &leaf.circuit);
        let events_targets = events_inputs.build_branch(
            &mut builder,
            mc,
            &leaf.events.indices,
            &unbounded_targets.left_proof,
            &unbounded_targets.right_proof,
        );

        let circuit = builder.build();

        let public_inputs = &circuit.prover_only.public_inputs;
        let unbounded = unbounded_targets.build(&leaf.unbounded, public_inputs);
        let events = events_targets.build(&leaf.events.indices, public_inputs);

        Self {
            unbounded,
            events,
            circuit,
        }
    }

    fn indices(&self) -> Indices {
        Indices {
            unbounded: self.unbounded.indices.clone(),
            events: self.events.indices,
        }
    }

    fn prove_helper<L: IsLeaf, R: IsLeaf>(
        &self,
        merge: &merge::BranchProof<F, C, D>,
        left_proof: &Proof<L, F, C, D>,
        right_proof: &Proof<R, F, C, D>,
        partial: bool,
    ) -> Result<BranchProof<F, C, D>> {
        let mut inputs = PartialWitness::new();
        self.unbounded.set_witness(
            &mut inputs,
            L::VALUE,
            &left_proof.proof,
            R::VALUE,
            &right_proof.proof,
        );
        self.events.set_witness(&mut inputs, partial, merge);
        let proof = self.circuit.prove(inputs)?;
        Ok(BranchProof {
            proof,
            tag: PhantomData,
            indices: self.indices(),
        })
    }

    pub fn prove<L: IsLeaf, R: IsLeaf>(
        &self,
        merge: &merge::BranchProof<F, C, D>,
        left_proof: &Proof<L, F, C, D>,
        right_proof: &Proof<R, F, C, D>,
    ) -> Result<BranchProof<F, C, D>> {
        self.prove_helper(merge, left_proof, right_proof, false)
    }

    pub fn prove_one<L: IsLeaf>(
        &self,
        merge: &merge::BranchProof<F, C, D>,
        left_proof: &Proof<L, F, C, D>,
    ) -> Result<BranchProof<F, C, D>> {
        self.prove_helper(merge, left_proof, left_proof, true)
    }

    pub fn verify(&self, proof: BranchProof<F, C, D>) -> Result<()> {
        self.circuit.verify(proof.proof)
    }
}

#[cfg(test)]
pub mod test {
    use super::*;
    use crate::circuits::merge::test as merge;
    use crate::circuits::test_data::{T0_HASH, T0_T1_HASH, T1_HASH};
    use crate::circuits::verify_program::test as verify_program;
    use crate::test_utils::{C, CONFIG, D, F};

    #[tested_fixture::tested_fixture(pub LEAF)]
    fn build_leaf() -> LeafCircuit<F, C, D> { LeafCircuit::new(&CONFIG, &verify_program::BRANCH) }

    #[tested_fixture::tested_fixture(pub BRANCH)]
    fn build_branch() -> BranchCircuit<F, C, D> {
        BranchCircuit::new(&CONFIG, &merge::BRANCH, &LEAF)
    }

    fn assert_proof<T>(proof: &Proof<T, F, C, D>, event_hash: Option<HashOut<F>>) {
        let indices = &LEAF.events.indices;
        assert_eq!(*indices, BRANCH.events.indices);

        let p_present = proof.events_present();
        assert_eq!(p_present, event_hash.is_some());
        let p_hash = proof.events();
        assert_eq!(p_hash, event_hash.unwrap_or_default());
    }

    #[tested_fixture::tested_fixture(pub T0_LEAF_PROOF: LeafProof<F, C, D>)]
    fn verify_t0_leaf() -> Result<LeafProof<F, C, D>> {
        let proof = LEAF.prove(&BRANCH, &verify_program::T0_BRANCH_PROOF)?;
        assert_proof(&proof, Some(*T0_HASH));
        LEAF.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(pub T1_LEAF_PROOF: LeafProof<F, C, D>)]
    fn verify_t1_leaf() -> Result<LeafProof<F, C, D>> {
        let proof = LEAF.prove(&BRANCH, &verify_program::T1_BRANCH_PROOF)?;
        assert_proof(&proof, Some(*T1_HASH));
        LEAF.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(pub BRANCH_PROOF: BranchProof<F, C, D>)]
    fn verify_branch() -> Result<BranchProof<F, C, D>> {
        let proof = BRANCH.prove(&merge::T0_T1_BRANCH_PROOF, &T0_LEAF_PROOF, &T1_LEAF_PROOF)?;
        assert_proof(&proof, Some(*T0_T1_HASH));
        BRANCH.verify(proof.clone())?;
        Ok(proof)
    }

    #[test]
    fn verify_partial_branch() -> Result<()> {
        let proof = BRANCH.prove_one(&merge::T1_PARTIAL_BRANCH_PROOF, &T1_LEAF_PROOF)?;
        assert_proof(&proof, Some(*T1_HASH));
        BRANCH.verify(proof.clone())?;
        Ok(())
    }
}

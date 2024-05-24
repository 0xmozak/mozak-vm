//! Circuits for recursively proving the merge of two binary merkle trees.
//!
//! The resulting merge of trees A and B will provably contain all nodes from A
//! and B and those nodes will retain their original relative positioning within
//! a tree, i.e. if A1 was to the left of A2 in the original tree, it will still
//! be in the resulting tree. However no order is defined for the positioning of
//! nodes between A and B, i.e. A1 could be to the left or right of B1.

use std::marker::PhantomData;

use anyhow::Result;
use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::{HashOut, RichField};
use plonky2::iop::witness::PartialWitness;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::{CircuitConfig, CircuitData, VerifierOnlyCircuitData};
use plonky2::plonk::config::{AlgebraicHasher, GenericConfig, Hasher};

use super::{Branch, Leaf};
use crate::subcircuits::unbounded;

pub mod core;
pub mod embed;

#[derive(Clone)]
pub struct Indices {
    pub unbounded: unbounded::PublicIndices,
    pub merge: core::PublicIndices,
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

    pub fn a_present(&self) -> bool {
        self.indices
            .merge
            .a_present
            .get_field(&self.proof.public_inputs)
    }

    pub fn a_hash(&self) -> HashOut<F> {
        self.indices
            .merge
            .a_hash
            .get_field(&self.proof.public_inputs)
    }

    pub fn b_present(&self) -> bool {
        self.indices
            .merge
            .b_present
            .get_field(&self.proof.public_inputs)
    }

    pub fn b_hash(&self) -> HashOut<F> {
        self.indices
            .merge
            .b_hash
            .get_field(&self.proof.public_inputs)
    }

    pub fn merged_present(&self) -> bool {
        self.indices
            .merge
            .merged_present
            .get_field(&self.proof.public_inputs)
    }

    pub fn merged_hash(&self) -> HashOut<F> {
        self.indices
            .merge
            .merged_hash
            .get_field(&self.proof.public_inputs)
    }
}

pub struct LeafCircuit<F, C, const D: usize>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>, {
    /// The recursion subcircuit
    pub unbounded: unbounded::LeafSubCircuit,

    /// The merge subcircuit
    pub merge: core::LeafSubCircuit,

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
        let merge_inputs = core::SubCircuitInputs::default(&mut builder);

        let unbounded_targets = unbounded_inputs.build_leaf::<F, C, D>(&mut builder);
        let merge_targets = merge_inputs.build_leaf(&mut builder);

        let circuit = builder.build();

        let public_inputs = &circuit.prover_only.public_inputs;
        let unbounded = unbounded_targets.build(public_inputs);
        let merge = merge_targets.build(public_inputs);

        Self {
            unbounded,
            merge,
            circuit,
        }
    }

    fn indices(&self) -> Indices {
        Indices {
            unbounded: self.unbounded.indices.clone(),
            merge: self.merge.indices,
        }
    }

    pub fn prove(
        &self,
        branch: &BranchCircuit<F, C, D>,
        a_hash: Option<HashOut<F>>,
        b_hash: Option<HashOut<F>>,
    ) -> Result<LeafProof<F, C, D>> {
        let mut inputs = PartialWitness::new();
        self.unbounded.set_witness(&mut inputs, &branch.circuit);
        self.merge.set_witness(&mut inputs, a_hash, b_hash);
        let proof = self.circuit.prove(inputs)?;
        Ok(LeafProof {
            proof,
            tag: PhantomData,
            indices: self.indices(),
        })
    }

    pub fn prove_unsafe(
        &self,
        branch: &BranchCircuit<F, C, D>,
        a_hash: Option<HashOut<F>>,
        b_hash: Option<HashOut<F>>,
        merged_hash: Option<HashOut<F>>,
    ) -> Result<LeafProof<F, C, D>> {
        let mut inputs = PartialWitness::new();
        self.unbounded.set_witness(&mut inputs, &branch.circuit);
        self.merge.set_witness_unsafe(
            &mut inputs,
            a_hash.is_some(),
            a_hash.unwrap_or_default(),
            b_hash.is_some(),
            b_hash.unwrap_or_default(),
            merged_hash.is_some(),
            merged_hash,
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

    /// The merge subcircuit
    pub merge: core::BranchSubCircuit,

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
        let merge_inputs = core::SubCircuitInputs::default(&mut builder);

        let unbounded_targets =
            unbounded_inputs.build_branch(&mut builder, &leaf.unbounded, &leaf.circuit);
        let merge_targets = merge_inputs.build_branch(
            &mut builder,
            &leaf.merge.indices,
            &unbounded_targets.left_proof,
            &unbounded_targets.right_proof,
        );

        let circuit = builder.build();

        let public_inputs = &circuit.prover_only.public_inputs;
        let unbounded = unbounded_targets.build(&leaf.unbounded, public_inputs);
        let merge = merge_targets.build(&leaf.merge.indices, public_inputs);

        Self {
            unbounded,
            merge,
            circuit,
        }
    }

    fn indices(&self) -> Indices {
        Indices {
            unbounded: self.unbounded.indices.clone(),
            merge: self.merge.indices,
        }
    }

    pub fn prove<'a>(
        &self,
        left_proof: impl Into<LeafOrBranchRef<'a, F, C, D>>,
        right_proof: impl Into<LeafOrBranchRef<'a, F, C, D>>,
    ) -> Result<BranchProof<F, C, D>>
    where
        C: 'a, {
        let (left_proof, right_proof) = (left_proof.into(), right_proof.into());
        let mut inputs = PartialWitness::new();
        self.unbounded.set_witness(
            &mut inputs,
            left_proof.is_leaf(),
            left_proof.proof(),
            right_proof.is_leaf(),
            right_proof.proof(),
        );
        self.merge.set_witness(&mut inputs);
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
    use anyhow::Ok;

    use super::*;
    pub use super::{BranchCircuit, BranchProof};
    use crate::circuits::test_data::{
        T0_A_HASH, T0_C_HASH, T0_HASH, T0_P0_HASH, T0_P2_A_HASH, T0_P2_C_HASH, T0_P2_HASH,
        T0_PM_HASH, T0_PM_P0_HASH, T0_T1_A_HASH, T0_T1_BCD_HASH, T0_T1_BC_HASH, T0_T1_HASH,
        T1_BD_HASH, T1_B_HASH, T1_HASH, T1_P1_HASH, T1_P2_A_HASH, T1_P2_D_HASH, T1_P2_HASH,
        T1_PM_HASH,
    };
    use crate::test_utils::{hash_branch, C, CONFIG, D, F, NON_ZERO_HASHES, ZERO_HASH};

    fn assert_leaf(proof: &LeafProof<F, C, D>, merged: Option<HashOut<F>>) {
        let p_present = proof.merged_present();
        assert_eq!(p_present, merged.is_some());

        let p_merged = proof.merged_hash();
        assert_eq!(p_merged, merged.unwrap_or_default());
    }

    fn assert_branch(
        proof: &BranchProof<F, C, D>,
        a_hash: Option<HashOut<F>>,
        b_hash: Option<HashOut<F>>,
        merged: Option<HashOut<F>>,
    ) {
        let p_a_present = proof.a_present();
        assert_eq!(p_a_present, a_hash.is_some());

        let p_a_hash = proof.a_hash();
        assert_eq!(p_a_hash, a_hash.unwrap_or_default());

        let p_b_present = proof.b_present();
        assert_eq!(p_b_present, b_hash.is_some());

        let p_b_hash = proof.b_hash();
        assert_eq!(p_b_hash, b_hash.unwrap_or_default());

        let p_merged_present = proof.merged_present();
        assert_eq!(p_merged_present, merged.is_some());

        let p_merged = proof.merged_hash();
        assert_eq!(p_merged, merged.unwrap_or_default());
    }

    #[tested_fixture::tested_fixture(pub LEAF)]
    fn build_leaf() -> LeafCircuit<F, C, D> { LeafCircuit::new(&CONFIG) }

    #[tested_fixture::tested_fixture(pub BRANCH)]
    fn build_branch() -> BranchCircuit<F, C, D> { BranchCircuit::new(&CONFIG, &LEAF) }

    #[tested_fixture::tested_fixture(EMPTY_LEAF_PROOF: LeafProof<F, C, D>)]
    fn verify_empty_leaf() -> Result<LeafProof<F, C, D>> {
        let proof = LEAF.prove(&BRANCH, None, None)?;
        assert_leaf(&proof, None);
        LEAF.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(LEFT_ZERO_LEAF_PROOF: LeafProof<F, C, D>)]
    fn verify_left_zero_leaf() -> Result<LeafProof<F, C, D>> {
        let proof = LEAF.prove(&BRANCH, Some(ZERO_HASH), None)?;
        assert_leaf(&proof, Some(ZERO_HASH));
        LEAF.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(RIGHT_ZERO_LEAF_PROOF: LeafProof<F, C, D>)]
    fn verify_right_zero_leaf() -> Result<LeafProof<F, C, D>> {
        let proof = LEAF.prove(&BRANCH, None, Some(ZERO_HASH))?;
        assert_leaf(&proof, Some(ZERO_HASH));
        LEAF.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(LEFT_LEAF_PROOF: LeafProof<F, C, D>)]
    fn verify_left_leaf() -> Result<LeafProof<F, C, D>> {
        let proof = LEAF.prove(&BRANCH, Some(NON_ZERO_HASHES[0]), None)?;
        assert_leaf(&proof, Some(NON_ZERO_HASHES[0]));
        LEAF.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(RIGHT_LEAF_PROOF: LeafProof<F, C, D>)]
    fn verify_right_leaf() -> Result<LeafProof<F, C, D>> {
        let proof = LEAF.prove(&BRANCH, None, Some(NON_ZERO_HASHES[1]))?;
        assert_leaf(&proof, Some(NON_ZERO_HASHES[1]));
        LEAF.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(pub EMPTY_BRANCH_PROOF: BranchProof<F, C, D>)]
    fn verify_empty_branch() -> Result<BranchProof<F, C, D>> {
        let proof = BRANCH.prove(*EMPTY_LEAF_PROOF, *EMPTY_LEAF_PROOF)?;
        assert_branch(&proof, None, None, None);
        BRANCH.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(pub LEFT_ZERO_BRANCH_PROOF: BranchProof<F, C, D>)]
    fn verify_left_zero_branch_1() -> Result<BranchProof<F, C, D>> {
        let proof = BRANCH.prove(*LEFT_ZERO_LEAF_PROOF, *EMPTY_LEAF_PROOF)?;
        assert_branch(&proof, Some(ZERO_HASH), None, Some(ZERO_HASH));
        BRANCH.verify(proof.clone())?;
        Ok(proof)
    }

    #[test]
    fn verify_left_zero_branch_2() -> Result<()> {
        let proof = BRANCH.prove(*EMPTY_LEAF_PROOF, *LEFT_ZERO_LEAF_PROOF)?;
        assert_branch(&proof, Some(ZERO_HASH), None, Some(ZERO_HASH));
        BRANCH.verify(proof)?;
        Ok(())
    }

    #[tested_fixture::tested_fixture(pub RIGHT_ZERO_BRANCH_PROOF: BranchProof<F, C, D>)]
    fn verify_right_zero_branch_1() -> Result<BranchProof<F, C, D>> {
        let proof = BRANCH.prove(*RIGHT_ZERO_LEAF_PROOF, *EMPTY_LEAF_PROOF)?;
        assert_branch(&proof, None, Some(ZERO_HASH), Some(ZERO_HASH));
        BRANCH.verify(proof.clone())?;
        Ok(proof)
    }

    #[test]
    fn verify_right_zero_branch_2() -> Result<()> {
        let proof = BRANCH.prove(*EMPTY_LEAF_PROOF, *RIGHT_ZERO_LEAF_PROOF)?;
        assert_branch(&proof, None, Some(ZERO_HASH), Some(ZERO_HASH));
        BRANCH.verify(proof)?;
        Ok(())
    }

    #[tested_fixture::tested_fixture(pub LEFT_BRANCH_PROOF: BranchProof<F, C, D>)]
    fn verify_left_branch_1() -> Result<BranchProof<F, C, D>> {
        let proof = BRANCH.prove(*LEFT_LEAF_PROOF, *EMPTY_LEAF_PROOF)?;
        assert_branch(
            &proof,
            Some(NON_ZERO_HASHES[0]),
            None,
            Some(NON_ZERO_HASHES[0]),
        );
        BRANCH.verify(proof.clone())?;
        Ok(proof)
    }

    #[test]
    fn verify_left_branch_2() -> Result<()> {
        let proof = BRANCH.prove(*EMPTY_LEAF_PROOF, *LEFT_LEAF_PROOF)?;
        assert_branch(
            &proof,
            Some(NON_ZERO_HASHES[0]),
            None,
            Some(NON_ZERO_HASHES[0]),
        );
        BRANCH.verify(proof)?;
        Ok(())
    }

    #[tested_fixture::tested_fixture(pub RIGHT_BRANCH_PROOF: BranchProof<F, C, D>)]
    fn verify_right_branch() -> Result<BranchProof<F, C, D>> {
        let proof = BRANCH.prove(*RIGHT_LEAF_PROOF, *EMPTY_LEAF_PROOF)?;
        assert_branch(
            &proof,
            None,
            Some(NON_ZERO_HASHES[1]),
            Some(NON_ZERO_HASHES[1]),
        );
        BRANCH.verify(proof.clone())?;
        Ok(proof)
    }

    #[test]
    fn verify_right_branch_2() -> Result<()> {
        let proof = BRANCH.prove(*EMPTY_LEAF_PROOF, *RIGHT_LEAF_PROOF)?;
        assert_branch(
            &proof,
            None,
            Some(NON_ZERO_HASHES[1]),
            Some(NON_ZERO_HASHES[1]),
        );
        BRANCH.verify(proof.clone())?;
        Ok(())
    }

    #[tested_fixture::tested_fixture(pub BOTH_BRANCH_PROOF: BranchProof<F, C, D>)]
    fn verify_both_branch() -> Result<BranchProof<F, C, D>> {
        let merged = hash_branch(&NON_ZERO_HASHES[0], &NON_ZERO_HASHES[1]);
        let proof = BRANCH.prove(*LEFT_LEAF_PROOF, *RIGHT_LEAF_PROOF)?;
        assert_branch(
            &proof,
            Some(NON_ZERO_HASHES[0]),
            Some(NON_ZERO_HASHES[1]),
            Some(merged),
        );
        BRANCH.verify(proof.clone())?;
        Ok(proof)
    }

    // T0 merges

    #[tested_fixture::tested_fixture(T0_PM_LEFT_LEAF_PROOF: LeafProof<F, C, D>)]
    fn verify_t0_pm_left_leaf() -> Result<LeafProof<F, C, D>> {
        let proof = LEAF.prove(&BRANCH, Some(*T0_PM_HASH), None)?;
        assert_leaf(&proof, Some(*T0_PM_HASH));
        LEAF.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(T0_P0_RIGHT_LEAF_PROOF: LeafProof<F, C, D>)]
    fn verify_t0_p0_right_leaf() -> Result<LeafProof<F, C, D>> {
        let proof = LEAF.prove(&BRANCH, None, Some(*T0_P0_HASH))?;
        assert_leaf(&proof, Some(*T0_P0_HASH));
        LEAF.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(T0_A_LEAF_PROOF: LeafProof<F, C, D>)]
    fn verify_t0_a_leaf() -> Result<LeafProof<F, C, D>> {
        let proof = LEAF.prove(&BRANCH, Some(*T0_P0_HASH), Some(*T0_P2_A_HASH))?;
        assert_leaf(&proof, Some(*T0_A_HASH));
        LEAF.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(T0_C_LEAF_PROOF: LeafProof<F, C, D>)]
    fn verify_t0_c_leaf() -> Result<LeafProof<F, C, D>> {
        let proof = LEAF.prove(&BRANCH, Some(*T0_PM_HASH), Some(*T0_P2_C_HASH))?;
        assert_leaf(&proof, Some(*T0_C_HASH));
        LEAF.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(pub T0_PM_P0_BRANCH_PROOF: BranchProof<F, C, D>)]
    fn verify_t0_pm_p0_branch() -> Result<BranchProof<F, C, D>> {
        // This is a simple merge because:
        // P0 contains only A and
        // PM contains only C
        // Also we put P0 to the left of PM because A < C
        let proof = BRANCH.prove(*T0_P0_RIGHT_LEAF_PROOF, *T0_PM_LEFT_LEAF_PROOF)?;
        assert_branch(
            &proof,
            Some(*T0_PM_HASH),
            Some(*T0_P0_HASH),
            Some(*T0_PM_P0_HASH),
        );
        BRANCH.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(pub T0_BRANCH_PROOF: BranchProof<F, C, D>)]
    fn verify_t0_branch() -> Result<BranchProof<F, C, D>> {
        // Merge A to the left of C because A < C
        let left_merged = hash_branch(&T0_P0_HASH, &T0_PM_HASH);
        let proof = BRANCH.prove(*T0_A_LEAF_PROOF, *T0_C_LEAF_PROOF)?;
        assert_branch(&proof, Some(left_merged), Some(*T0_P2_HASH), Some(*T0_HASH));
        Ok(proof)
    }

    // T1 merges

    #[tested_fixture::tested_fixture(pub T1_PM_P1_BRANCH_PROOF: BranchProof<F, C, D>)]
    fn verify_t1_pm_p1_branch() -> Result<BranchProof<F, C, D>> {
        // This is a simple merge because:
        // PM contains only B and
        // P1 contains only B and
        // We put PM to the left arbitrarily
        // This means we can do this all in a single leaf proof
        let proof = LEAF.prove(&BRANCH, Some(*T1_PM_HASH), Some(*T1_P1_HASH))?;
        assert_leaf(&proof, Some(*T1_B_HASH));
        LEAF.verify(proof.clone())?;

        // But since the result must be a branch, just merge with an empty branch
        let proof = BRANCH.prove(&proof, *EMPTY_LEAF_PROOF)?;
        assert_branch(
            &proof,
            Some(*T1_PM_HASH),
            Some(*T1_P1_HASH),
            Some(*T1_B_HASH),
        );
        BRANCH.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(T1_B_LEFT_LEAF_PROOF: LeafProof<F, C, D>)]
    fn verify_t1_b_left_leaf() -> Result<LeafProof<F, C, D>> {
        let proof = LEAF.prove(&BRANCH, Some(*T1_B_HASH), None)?;
        assert_leaf(&proof, Some(*T1_B_HASH));
        LEAF.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(T1_P2_A_RIGHT_LEAF_PROOF: LeafProof<F, C, D>)]
    fn verify_t1_p2_a_right_leaf() -> Result<LeafProof<F, C, D>> {
        let proof = LEAF.prove(&BRANCH, None, Some(*T1_P2_A_HASH))?;
        assert_leaf(&proof, Some(*T1_P2_A_HASH));
        LEAF.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(T1_P2_D_RIGHT_LEAF_PROOF: LeafProof<F, C, D>)]
    fn verify_t1_p2_d_right_leaf() -> Result<LeafProof<F, C, D>> {
        let proof = LEAF.prove(&BRANCH, None, Some(*T1_P2_D_HASH))?;
        assert_leaf(&proof, Some(*T1_P2_D_HASH));
        LEAF.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(T1_BD_BRANCH_PROOF: BranchProof<F, C, D>)]
    fn verify_t1_bd_branch() -> Result<BranchProof<F, C, D>> {
        // Merge B to the left of D because B < D
        let proof = BRANCH.prove(*T1_B_LEFT_LEAF_PROOF, *T1_P2_D_RIGHT_LEAF_PROOF)?;
        assert_branch(
            &proof,
            Some(*T1_B_HASH),
            Some(*T1_P2_D_HASH),
            Some(*T1_BD_HASH),
        );
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(pub T1_BRANCH_PROOF: BranchProof<F, C, D>)]
    fn verify_t1_branch() -> Result<BranchProof<F, C, D>> {
        // Merge A to the left of BD because A < BD
        let proof = BRANCH.prove(*T1_P2_A_RIGHT_LEAF_PROOF, *T1_BD_BRANCH_PROOF)?;
        assert_branch(&proof, Some(*T1_B_HASH), Some(*T1_P2_HASH), Some(*T1_HASH));
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(T1_P2_PARTIAL_LEAF_PROOF: LeafProof<F, C, D>)]
    fn verify_t1_p2_partial_leaf() -> Result<LeafProof<F, C, D>> {
        let proof = LEAF.prove(&BRANCH, Some(*T1_P2_HASH), None)?;
        assert_leaf(&proof, Some(*T1_P2_HASH));
        LEAF.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(pub T1_P2_PARTIAL_BRANCH_PROOF: BranchProof<F, C, D>)]
    fn verify_t1_p2_partial_branch() -> Result<BranchProof<F, C, D>> {
        let proof = BRANCH.prove(*T1_P2_PARTIAL_LEAF_PROOF, *EMPTY_LEAF_PROOF)?;
        assert_branch(&proof, Some(*T1_P2_HASH), None, Some(*T1_P2_HASH));
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(pub T1_PM_P1_PARTIAL_BRANCH_PROOF: BranchProof<F, C, D>)]
    fn verify_t1_pm_p1_partial_branch() -> Result<BranchProof<F, C, D>> {
        let proof = BRANCH.prove(*T1_B_LEFT_LEAF_PROOF, *EMPTY_LEAF_PROOF)?;
        assert_branch(&proof, Some(*T1_B_HASH), None, Some(*T1_B_HASH));
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(T1_PARTIAL_LEAF_PROOF: LeafProof<F, C, D>)]
    fn verify_t1_partial_leaf() -> Result<LeafProof<F, C, D>> {
        let proof = LEAF.prove(&BRANCH, Some(*T1_HASH), None)?;
        assert_leaf(&proof, Some(*T1_HASH));
        LEAF.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(pub T1_PARTIAL_BRANCH_PROOF: BranchProof<F, C, D>)]
    fn verify_t1_partial_branch() -> Result<BranchProof<F, C, D>> {
        let proof = BRANCH.prove(*T1_PARTIAL_LEAF_PROOF, *EMPTY_LEAF_PROOF)?;
        assert_branch(&proof, Some(*T1_HASH), None, Some(*T1_HASH));
        Ok(proof)
    }

    // Merge transactions

    #[tested_fixture::tested_fixture(T0_T1_A_LEAF_PROOF: LeafProof<F, C, D>)]
    fn verify_t0_t1_a_leaf() -> Result<LeafProof<F, C, D>> {
        let proof = LEAF.prove(&BRANCH, Some(*T0_A_HASH), Some(*T1_P2_A_HASH))?;
        assert_leaf(&proof, Some(*T0_T1_A_HASH));
        LEAF.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(X_T1_B_LEAF_PROOF: LeafProof<F, C, D>)]
    fn verify_x_t1_b_leaf() -> Result<LeafProof<F, C, D>> {
        let proof = LEAF.prove(&BRANCH, None, Some(*T1_B_HASH))?;
        assert_leaf(&proof, Some(*T1_B_HASH));
        LEAF.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(T0_X_C_LEAF_PROOF: LeafProof<F, C, D>)]
    fn verify_t0_x_c_leaf() -> Result<LeafProof<F, C, D>> {
        let proof = LEAF.prove(&BRANCH, Some(*T0_C_HASH), None)?;
        assert_leaf(&proof, Some(*T0_C_HASH));
        LEAF.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(X_T1_D_LEAF_PROOF: LeafProof<F, C, D>)]
    fn verify_x_t1_d_leaf() -> Result<LeafProof<F, C, D>> {
        let proof = LEAF.prove(&BRANCH, None, Some(*T1_P2_D_HASH))?;
        assert_leaf(&proof, Some(*T1_P2_D_HASH));
        LEAF.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(T0_T1_BC_BRANCH_PROOF: BranchProof<F, C, D>)]
    fn verify_t0_t1_bc_branch() -> Result<BranchProof<F, C, D>> {
        let proof = BRANCH.prove(*X_T1_B_LEAF_PROOF, *T0_X_C_LEAF_PROOF)?;
        assert_branch(
            &proof,
            Some(*T0_C_HASH),
            Some(*T1_B_HASH),
            Some(*T0_T1_BC_HASH),
        );
        BRANCH.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(T0_T1_BCD_BRANCH_PROOF: BranchProof<F, C, D>)]
    fn verify_t0_t1_bcd_leaf() -> Result<BranchProof<F, C, D>> {
        let proof = BRANCH.prove(*T0_T1_BC_BRANCH_PROOF, *X_T1_D_LEAF_PROOF)?;
        assert_branch(
            &proof,
            Some(*T0_C_HASH),
            Some(*T1_BD_HASH),
            Some(*T0_T1_BCD_HASH),
        );
        BRANCH.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(pub T0_T1_BRANCH_PROOF: BranchProof<F, C, D>)]
    fn verify_t0_t1_branch() -> Result<BranchProof<F, C, D>> {
        let proof = BRANCH.prove(*T0_T1_A_LEAF_PROOF, *T0_T1_BCD_BRANCH_PROOF)?;
        assert_branch(&proof, Some(*T0_HASH), Some(*T1_HASH), Some(*T0_T1_HASH));
        BRANCH.verify(proof.clone())?;
        Ok(proof)
    }
}

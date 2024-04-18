//! Circuits for recursively proving the merge of two binary merkle trees.
//!
//! The resulting merge of trees A and B will provably contain all nodes from A
//! and B and those nodes will retain their original relative positioning within
//! a tree, i.e. if A1 was to the left of A2 in the original tree, it will still
//! be in the resulting tree. However no order is defined for the positioning of
//! nodes between A and B, i.e. A1 could be to the left or right of B1.

use anyhow::Result;
use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::{HashOut, RichField};
use plonky2::iop::witness::PartialWitness;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::{CircuitConfig, CircuitData};
use plonky2::plonk::config::{AlgebraicHasher, GenericConfig};
use plonky2::plonk::proof::ProofWithPublicInputs;

use crate::subcircuits::unbounded;

pub mod core;

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

    pub fn prove(
        &self,
        branch: &BranchCircuit<F, C, D>,
        a_hash: Option<HashOut<F>>,
        b_hash: Option<HashOut<F>>,
        merged_hash: Option<HashOut<F>>,
    ) -> Result<ProofWithPublicInputs<F, C, D>> {
        let mut inputs = PartialWitness::new();
        self.unbounded.set_witness(&mut inputs, &branch.circuit);
        self.merge
            .set_witness(&mut inputs, a_hash, b_hash, merged_hash);
        self.circuit.prove(inputs)
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

    pub fn prove(
        &self,
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
        self.circuit.prove(inputs)
    }
}

#[cfg(test)]
mod test {
    use lazy_static::lazy_static;
    use plonky2::field::types::Field;
    use plonky2::hash::hash_types::NUM_HASH_OUT_ELTS;

    use super::*;
    use crate::test_utils::{fast_test_circuit_config, hash_branch, hash_str, C, D, F};

    const CONFIG: CircuitConfig = fast_test_circuit_config();

    lazy_static! {
        static ref LEAF: LeafCircuit<F, C, D> = LeafCircuit::new(&CONFIG);
        static ref BRANCH: BranchCircuit<F, C, D> = BranchCircuit::new(&CONFIG, &LEAF);
    }

    #[test]
    fn verify_leaf_empty() -> Result<()> {
        let zero_hash = HashOut::from([F::ZERO; NUM_HASH_OUT_ELTS]);
        let a_val = hash_str("Value Alpha");
        let b_val = hash_str("Value Beta");

        let proof = LEAF.prove(&BRANCH, None, None, Some(zero_hash))?;
        LEAF.circuit.verify(proof)?;

        let proof = LEAF.prove(&BRANCH, Some(a_val), None, Some(a_val))?;
        LEAF.circuit.verify(proof)?;

        let proof = LEAF.prove(&BRANCH, None, Some(b_val), Some(b_val))?;
        LEAF.circuit.verify(proof)?;

        let proof = LEAF.prove(&BRANCH, Some(zero_hash), None, Some(zero_hash))?;
        LEAF.circuit.verify(proof)?;

        let proof = LEAF.prove(&BRANCH, None, Some(zero_hash), Some(zero_hash))?;
        LEAF.circuit.verify(proof)?;

        Ok(())
    }

    #[test]
    fn verify_leaf() -> Result<()> {
        let zero_hash = HashOut::from([F::ZERO; NUM_HASH_OUT_ELTS]);
        let a_val = hash_str("Value Alpha");
        let b_val = hash_str("Value Beta");
        let ab_hash = hash_branch(&a_val, &b_val);
        let zero_zero_hash = hash_branch(&zero_hash, &zero_hash);
        let a_zero_hash = hash_branch(&a_val, &zero_hash);
        let zero_b_hash = hash_branch(&zero_hash, &b_val);

        let proof = LEAF.prove(&BRANCH, Some(a_val), Some(b_val), Some(ab_hash))?;
        LEAF.circuit.verify(proof)?;

        let proof = LEAF.prove(
            &BRANCH,
            Some(zero_hash),
            Some(zero_hash),
            Some(zero_zero_hash),
        )?;
        LEAF.circuit.verify(proof)?;

        let proof = LEAF.prove(&BRANCH, Some(a_val), Some(zero_hash), Some(a_zero_hash))?;
        LEAF.circuit.verify(proof)?;

        let proof = LEAF.prove(&BRANCH, Some(zero_hash), Some(b_val), Some(zero_b_hash))?;
        LEAF.circuit.verify(proof)?;

        Ok(())
    }

    #[test]
    fn verify_branch_empty() -> Result<()> {
        let zero_hash = HashOut::from([F::ZERO; NUM_HASH_OUT_ELTS]);

        let empty_proof = LEAF.prove(&BRANCH, None, None, Some(zero_hash))?;
        LEAF.circuit.verify(empty_proof.clone())?;

        let empty_branch = BRANCH.prove(true, &empty_proof, true, &empty_proof)?;
        BRANCH.circuit.verify(empty_branch.clone())?;

        let empty_branch = BRANCH.prove(false, &empty_branch, true, &empty_proof)?;
        BRANCH.circuit.verify(empty_branch.clone())?;

        let empty_branch = BRANCH.prove(false, &empty_branch, false, &empty_branch)?;
        BRANCH.circuit.verify(empty_branch.clone())?;

        Ok(())
    }

    #[test]
    fn verify_branch_single() -> Result<()> {
        let a_val = hash_str("Value Alpha");
        let b_val = hash_str("Value Beta");

        let a_proof = LEAF.prove(&BRANCH, Some(a_val), None, Some(a_val))?;
        LEAF.circuit.verify(a_proof.clone())?;

        let b_proof = LEAF.prove(&BRANCH, None, Some(b_val), Some(b_val))?;
        LEAF.circuit.verify(b_proof.clone())?;

        let proof = BRANCH.prove(true, &a_proof, true, &b_proof)?;
        BRANCH.circuit.verify(proof.clone())?;

        Ok(())
    }
}

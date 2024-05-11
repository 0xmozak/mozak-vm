//! Circuits for proving updates to the state tree.

use anyhow::Result;
use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::{HashOut, HashOutTarget, RichField};
use plonky2::hash::poseidon2::Poseidon2Hash;
use plonky2::iop::witness::PartialWitness;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::{CircuitConfig, CircuitData};
use plonky2::plonk::config::{AlgebraicHasher, GenericConfig};
use plonky2::plonk::proof::ProofWithPublicInputs;

use crate::subcircuits::{bounded, summarized, unpruned, verify_address};
use crate::{at_least_one_true, hashes_equal};

pub struct LeafCircuit<F, C, const D: usize>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>, {
    pub bounded: bounded::LeafSubCircuit,
    pub summarized: summarized::LeafSubCircuit,
    pub old: unpruned::LeafSubCircuit,
    pub new: unpruned::LeafSubCircuit,
    pub address: verify_address::LeafSubCircuit,
    pub circuit: CircuitData<F, C, D>,
}

impl<F, C, const D: usize> LeafCircuit<F, C, D>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
{
    #[must_use]
    pub fn new(circuit_config: &CircuitConfig) -> Self {
        let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());

        let bounded_inputs = bounded::SubCircuitInputs::default(&mut builder);
        let summarized_inputs = summarized::SubCircuitInputs::default(&mut builder);
        let old_inputs = unpruned::SubCircuitInputs::default(&mut builder);
        let new_inputs = unpruned::SubCircuitInputs::default(&mut builder);
        let address_inputs = verify_address::SubCircuitInputs {
            node_address: builder.add_virtual_target(),
            node_present: summarized_inputs.summary_hash_present,
        };
        builder.register_public_input(address_inputs.node_address);

        let bounded_targets = bounded_inputs.build_leaf(&mut builder);
        let summarized_targets = summarized_inputs.build_leaf(&mut builder);
        let old_targets = old_inputs.build_leaf(&mut builder);
        let new_targets = new_inputs.build_leaf(&mut builder);
        let address_targets = address_inputs.build_leaf(&mut builder);

        let old_hash = old_targets.inputs.unpruned_hash;
        let new_hash = new_targets.inputs.unpruned_hash;

        let unchanged = hashes_equal(&mut builder, old_hash, new_hash);
        let summary_present = summarized_targets.inputs.summary_hash_present;

        // We can't be changed (unchanged == 0) and not-present in the summary
        at_least_one_true(&mut builder, [unchanged, summary_present]);

        // Make the observation
        let observation = [address_targets.node_address]
            .into_iter()
            .chain(old_hash.elements)
            .chain(new_hash.elements)
            .collect();
        let observation = builder.hash_n_to_hash_no_pad::<Poseidon2Hash>(observation);

        // zero it out based on if this node is being summarized
        let observation = observation
            .elements
            .map(|e| builder.mul(e, summary_present.target));

        // This should be the summary hash
        builder.connect_hashes(
            HashOutTarget::from(observation),
            summarized_targets.inputs.summary_hash,
        );

        let circuit = builder.build();

        let public_inputs = &circuit.prover_only.public_inputs;
        let bounded = bounded_targets.build(public_inputs);
        let summarized = summarized_targets.build(public_inputs);
        let old = old_targets.build(public_inputs);
        let new = new_targets.build(public_inputs);
        let address = address_targets.build(public_inputs);

        Self {
            bounded,
            summarized,
            old,
            new,
            address,
            circuit,
        }
    }

    pub fn prove(
        &self,
        old_hash: HashOut<F>,
        new_hash: HashOut<F>,
        address: Option<u64>,
    ) -> Result<ProofWithPublicInputs<F, C, D>> {
        let mut inputs = PartialWitness::new();
        self.bounded.set_witness(&mut inputs);
        self.old.set_witness(&mut inputs, old_hash);
        self.new.set_witness(&mut inputs, new_hash);
        self.address.set_witness(&mut inputs, address);
        self.circuit.prove(inputs)
    }

    pub fn prove_unsafe(
        &self,
        old_hash: HashOut<F>,
        new_hash: HashOut<F>,
        summarized: Option<HashOut<F>>,
        address: Option<u64>,
    ) -> Result<ProofWithPublicInputs<F, C, D>> {
        let mut inputs = PartialWitness::new();
        self.bounded.set_witness(&mut inputs);
        self.summarized.set_witness(&mut inputs, summarized);
        self.old.set_witness(&mut inputs, old_hash);
        self.new.set_witness(&mut inputs, new_hash);
        self.address.set_witness(&mut inputs, address);
        self.circuit.prove(inputs)
    }
}

pub struct BranchCircuit<F, C, const D: usize>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>, {
    pub bounded: bounded::BranchSubCircuit<D>,
    pub summarized: summarized::BranchSubCircuit,
    pub old: unpruned::BranchSubCircuit,
    pub new: unpruned::BranchSubCircuit,
    pub address: verify_address::BranchSubCircuit,
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
        summarized_indicies: &summarized::PublicIndices,
        old_indicies: &unpruned::PublicIndices,
        new_indicies: &unpruned::PublicIndices,
        address_indicies: &verify_address::PublicIndices,
        child: &CircuitData<F, C, D>,
    ) -> Self {
        let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());

        let bounded_inputs = bounded::SubCircuitInputs::default(&mut builder);
        let summarized_inputs = summarized::SubCircuitInputs::default(&mut builder);
        let old_inputs = unpruned::SubCircuitInputs::default(&mut builder);
        let new_inputs = unpruned::SubCircuitInputs::default(&mut builder);
        let address_inputs = verify_address::SubCircuitInputs {
            node_address: builder.add_virtual_target(),
            node_present: summarized_inputs.summary_hash_present,
        };
        builder.register_public_input(address_inputs.node_address);

        let bounded_targets = bounded_inputs.build_branch(&mut builder, child);
        let summarized_targets = summarized_inputs.build_branch(
            &mut builder,
            summarized_indicies,
            &bounded_targets.left_proof,
            &bounded_targets.right_proof,
        );
        let old_targets = old_inputs.build_branch(
            &mut builder,
            old_indicies,
            &bounded_targets.left_proof,
            &bounded_targets.right_proof,
            false,
        );
        let new_targets = new_inputs.build_branch(
            &mut builder,
            new_indicies,
            &bounded_targets.left_proof,
            &bounded_targets.right_proof,
            false,
        );
        let address_targets = address_inputs.build_branch(
            &mut builder,
            address_indicies,
            &bounded_targets.left_proof,
            &bounded_targets.right_proof,
        );

        let circuit = builder.build();

        let public_inputs = &circuit.prover_only.public_inputs;
        let bounded = bounded_targets.build(public_inputs);
        let summarized = summarized_targets.build(summarized_indicies, public_inputs);
        let old = old_targets.build(old_indicies, public_inputs);
        let new = new_targets.build(new_indicies, public_inputs);
        let address = address_targets.build(address_indicies, public_inputs);

        Self {
            bounded,
            summarized,
            old,
            new,
            address,
            circuit,
        }
    }

    #[must_use]
    pub fn from_leaf(circuit_config: &CircuitConfig, leaf: &LeafCircuit<F, C, D>) -> Self {
        Self::new(
            circuit_config,
            &leaf.summarized.indices,
            &leaf.old.indices,
            &leaf.new.indices,
            &leaf.address.indices,
            &leaf.circuit,
        )
    }

    #[must_use]
    pub fn from_branch(circuit_config: &CircuitConfig, branch: &Self) -> Self {
        Self::new(
            circuit_config,
            &branch.summarized.indices,
            &branch.old.indices,
            &branch.new.indices,
            &branch.address.indices,
            &branch.circuit,
        )
    }

    pub fn prove(
        &self,
        left_proof: &ProofWithPublicInputs<F, C, D>,
        right_proof: &ProofWithPublicInputs<F, C, D>,
    ) -> Result<ProofWithPublicInputs<F, C, D>>
    where
        <C as GenericConfig<D>>::Hasher: AlgebraicHasher<F>, {
        let mut inputs = PartialWitness::new();
        self.bounded
            .set_witness(&mut inputs, left_proof, right_proof);
        self.summarized.set_witness(&mut inputs);
        self.circuit.prove(inputs)
    }

    pub fn prove_unsafe(
        &self,
        old_hash: Option<HashOut<F>>,
        new_hash: Option<HashOut<F>>,
        summary_hash: Option<Option<HashOut<F>>>,
        address: impl Into<AddressPresent>,
        left_proof: &ProofWithPublicInputs<F, C, D>,
        right_proof: &ProofWithPublicInputs<F, C, D>,
    ) -> Result<ProofWithPublicInputs<F, C, D>>
    where
        <C as GenericConfig<D>>::Hasher: AlgebraicHasher<F>, {
        let mut inputs = PartialWitness::new();
        self.bounded
            .set_witness(&mut inputs, left_proof, right_proof);
        if let Some(summary_hash) = summary_hash {
            self.summarized.set_witness_unsafe(
                &mut inputs,
                summary_hash.is_some(),
                summary_hash.unwrap_or_default(),
            );
        }
        if let Some(old_hash) = old_hash {
            self.old.set_witness_unsafe(&mut inputs, old_hash);
        }
        if let Some(new_hash) = new_hash {
            self.new.set_witness_unsafe(&mut inputs, new_hash);
        }
        match address.into() {
            AddressPresent::Present(a) => self.address.set_witness(&mut inputs, Some(a)),
            AddressPresent::Absent => self.address.set_witness(&mut inputs, None),
            AddressPresent::Implicit => {}
        }
        self.circuit.prove(inputs)
    }
}

pub enum AddressPresent {
    Present(u64),
    Absent,
    Implicit,
}

impl From<()> for AddressPresent {
    fn from(_value: ()) -> Self { Self::Implicit }
}

impl From<Option<u64>> for AddressPresent {
    fn from(value: Option<u64>) -> Self { value.map_or(Self::Absent, Self::Present) }
}
impl From<u64> for AddressPresent {
    fn from(value: u64) -> Self { Self::Present(value) }
}

#[cfg(test)]
pub mod test {
    use once_cell::sync::Lazy;
    use plonky2::field::types::Field;

    use super::*;
    use crate::circuits::test_data::{
        ADDRESS_A, ADDRESS_A_SUMMARY_HASH, ADDRESS_B, ADDRESS_BCD_SUMMARY_HASH,
        ADDRESS_BC_SUMMARY_HASH, ADDRESS_B_SUMMARY_HASH, ADDRESS_C, ADDRESS_C_SUMMARY_HASH,
        ADDRESS_D, ADDRESS_D_SUMMARY_HASH, ADDRESS_E, ROOT_SUMMARY_HASH, STATE_0_BRANCH_HASHES,
        STATE_0_DOUBLE_BRANCH_HASHES, STATE_0_LEAF_HASHES, STATE_0_ROOT_HASH,
        STATE_1_BRANCH_HASHES, STATE_1_DOUBLE_BRANCH_HASHES, STATE_1_LEAF_HASHES,
        STATE_1_ROOT_HASH, ZERO_OBJ_HASH,
    };
    use crate::summarize;
    use crate::test_utils::{hash_branch, C, CONFIG, D, F, NON_ZERO_HASHES, ZERO_HASH};

    static EMPTY_BRANCH_HASH: Lazy<HashOut<F>> =
        Lazy::new(|| hash_branch(&ZERO_OBJ_HASH, &ZERO_OBJ_HASH));

    #[tested_fixture::tested_fixture(LEAF)]
    fn build_leaf() -> LeafCircuit<F, C, D> { LeafCircuit::new(&CONFIG) }

    #[tested_fixture::tested_fixture(BRANCH_1)]
    fn build_branch_1() -> BranchCircuit<F, C, D> { BranchCircuit::from_leaf(&CONFIG, &LEAF) }

    #[tested_fixture::tested_fixture(BRANCH_2)]
    fn build_branch_2() -> BranchCircuit<F, C, D> { BranchCircuit::from_branch(&CONFIG, &BRANCH_1) }

    #[tested_fixture::tested_fixture(pub BRANCH_3)]
    fn build_branch_3() -> BranchCircuit<F, C, D> { BranchCircuit::from_branch(&CONFIG, &BRANCH_2) }

    fn assert_proof(
        proof: &ProofWithPublicInputs<F, C, D>,
        old_state: HashOut<F>,
        new_state: HashOut<F>,
        summary_address: Option<(HashOut<F>, u64)>,
    ) {
        let indices = &LEAF.old.indices;
        assert_eq!(indices, &BRANCH_1.old.indices);
        assert_eq!(indices, &BRANCH_2.old.indices);
        assert_eq!(indices, &BRANCH_3.old.indices);

        let p_old = indices.unpruned_hash.get_any(&proof.public_inputs);
        assert_eq!(p_old, old_state.elements);

        let indices = &LEAF.new.indices;
        assert_eq!(indices, &BRANCH_1.new.indices);
        assert_eq!(indices, &BRANCH_2.new.indices);
        assert_eq!(indices, &BRANCH_3.new.indices);

        let p_new = indices.unpruned_hash.get_any(&proof.public_inputs);
        assert_eq!(p_new, new_state.elements);

        let indices = &LEAF.summarized.indices;
        assert_eq!(indices, &BRANCH_1.summarized.indices);
        assert_eq!(indices, &BRANCH_2.summarized.indices);
        assert_eq!(indices, &BRANCH_3.summarized.indices);

        let p_summary_present = indices.summary_hash_present.get_any(&proof.public_inputs);
        assert_eq!(p_summary_present, F::from_bool(summary_address.is_some()));

        let p_summary = indices.summary_hash.get_any(&proof.public_inputs);
        assert_eq!(p_summary, summary_address.unwrap_or_default().0.elements);

        let indices = &LEAF.address.indices;
        assert_eq!(indices, &BRANCH_1.address.indices);
        assert_eq!(indices, &BRANCH_2.address.indices);
        assert_eq!(indices, &BRANCH_3.address.indices);

        let p_address = indices.node_address.get(&proof.public_inputs);
        assert_eq!(
            p_address,
            summary_address.map_or(F::NEG_ONE, |x| F::from_canonical_u64(x.1))
        );
    }

    #[tested_fixture::tested_fixture(EMPTY_LEAF_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_empty_leaf() -> Result<ProofWithPublicInputs<F, C, D>> {
        let proof = LEAF.prove(*ZERO_OBJ_HASH, *ZERO_OBJ_HASH, None)?;
        assert_proof(&proof, *ZERO_OBJ_HASH, *ZERO_OBJ_HASH, None);
        LEAF.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(A_UPDATE_LEAF_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_a_update_leaf() -> Result<ProofWithPublicInputs<F, C, D>> {
        let (old, new) = (
            STATE_0_LEAF_HASHES[ADDRESS_A],
            STATE_1_LEAF_HASHES[ADDRESS_A],
        );
        let a = ADDRESS_A as u64;
        let proof = LEAF.prove(old, new, Some(a))?;
        assert_proof(&proof, old, new, Some((*ADDRESS_A_SUMMARY_HASH, a)));
        LEAF.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(B_DELETE_LEAF_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_b_delete_leaf() -> Result<ProofWithPublicInputs<F, C, D>> {
        let (old, new) = (
            STATE_0_LEAF_HASHES[ADDRESS_B],
            STATE_1_LEAF_HASHES[ADDRESS_B],
        );
        let a = ADDRESS_B as u64;
        let proof = LEAF.prove(old, new, Some(a))?;
        assert_proof(&proof, old, new, Some((*ADDRESS_B_SUMMARY_HASH, a)));
        LEAF.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(C_CREATE_LEAF_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_c_create_leaf() -> Result<ProofWithPublicInputs<F, C, D>> {
        let (old, new) = (
            STATE_0_LEAF_HASHES[ADDRESS_C],
            STATE_1_LEAF_HASHES[ADDRESS_C],
        );
        let a = ADDRESS_C as u64;
        let proof = LEAF.prove(old, new, Some(a))?;
        assert_proof(&proof, old, new, Some((*ADDRESS_C_SUMMARY_HASH, a)));
        LEAF.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(D_READ_LEAF_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_d_read_leaf() -> Result<ProofWithPublicInputs<F, C, D>> {
        let (old, new) = (
            STATE_0_LEAF_HASHES[ADDRESS_D],
            STATE_1_LEAF_HASHES[ADDRESS_D],
        );
        assert_eq!(old, new);
        let a = ADDRESS_D as u64;
        let proof = LEAF.prove(old, new, Some(a))?;
        assert_proof(&proof, old, new, Some((*ADDRESS_D_SUMMARY_HASH, a)));
        LEAF.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(E_IGNORED_LEAF_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_e_ignored_leaf() -> Result<ProofWithPublicInputs<F, C, D>> {
        let (old, new) = (
            STATE_0_LEAF_HASHES[ADDRESS_E],
            STATE_1_LEAF_HASHES[ADDRESS_E],
        );
        assert_eq!(old, new);
        let proof = LEAF.prove(old, new, None)?;
        assert_proof(&proof, old, new, None);
        LEAF.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[test]
    #[should_panic(expected = "Tried to invert zero")]
    fn bad_leaf_create() {
        let proof = LEAF.prove(ZERO_HASH, NON_ZERO_HASHES[0], None).unwrap();
        LEAF.circuit.verify(proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_leaf_update() {
        let summary = summarize(42, ZERO_HASH, NON_ZERO_HASHES[1]);
        let proof = LEAF
            .prove_unsafe(
                NON_ZERO_HASHES[0],
                NON_ZERO_HASHES[1],
                Some(summary),
                Some(42),
            )
            .unwrap();
        LEAF.circuit.verify(proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_leaf_non_update() {
        let summary = summarize(42, ZERO_HASH, NON_ZERO_HASHES[0]);
        let proof = LEAF
            .prove_unsafe(
                NON_ZERO_HASHES[0],
                NON_ZERO_HASHES[0],
                Some(summary),
                Some(42),
            )
            .unwrap();
        LEAF.circuit.verify(proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_leaf_address() {
        let summary = summarize(41, ZERO_HASH, NON_ZERO_HASHES[0]);
        let proof = LEAF
            .prove_unsafe(ZERO_HASH, NON_ZERO_HASHES[0], Some(summary), Some(42))
            .unwrap();
        LEAF.circuit.verify(proof).unwrap();
    }

    #[tested_fixture::tested_fixture(EMPTY_BRANCH_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_empty_branch() -> Result<ProofWithPublicInputs<F, C, D>> {
        let proof = BRANCH_1.prove(&EMPTY_LEAF_PROOF, &EMPTY_LEAF_PROOF)?;
        assert_proof(&proof, *EMPTY_BRANCH_HASH, *EMPTY_BRANCH_HASH, None);
        BRANCH_1.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(A_UPDATE_BRANCH_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_a_update_branch() -> Result<ProofWithPublicInputs<F, C, D>> {
        let proof = BRANCH_1.prove(*A_UPDATE_LEAF_PROOF, &EMPTY_LEAF_PROOF)?;
        assert_proof(
            &proof,
            STATE_0_BRANCH_HASHES[ADDRESS_A / 2],
            STATE_1_BRANCH_HASHES[ADDRESS_A / 2],
            Some((*ADDRESS_A_SUMMARY_HASH, ADDRESS_A as u64 / 2)),
        );
        BRANCH_1.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(BC_DELETE_CREATE_BRANCH_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_bc_delete_create_branch() -> Result<ProofWithPublicInputs<F, C, D>> {
        let proof = BRANCH_1.prove(*B_DELETE_LEAF_PROOF, *C_CREATE_LEAF_PROOF)?;
        assert_eq!(ADDRESS_B / 2, ADDRESS_C / 2);
        assert_proof(
            &proof,
            STATE_0_BRANCH_HASHES[ADDRESS_B / 2],
            STATE_1_BRANCH_HASHES[ADDRESS_B / 2],
            Some((*ADDRESS_BC_SUMMARY_HASH, ADDRESS_B as u64 / 2)),
        );
        BRANCH_1.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(DE_READ_IGNORED_BRANCH_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_de_read_ignored_branch() -> Result<ProofWithPublicInputs<F, C, D>> {
        let proof = BRANCH_1.prove(*D_READ_LEAF_PROOF, *E_IGNORED_LEAF_PROOF)?;
        assert_eq!(ADDRESS_D / 2, ADDRESS_E / 2);
        assert_proof(
            &proof,
            STATE_0_BRANCH_HASHES[ADDRESS_D / 2],
            STATE_1_BRANCH_HASHES[ADDRESS_D / 2],
            Some((*ADDRESS_D_SUMMARY_HASH, ADDRESS_D as u64 / 2)),
        );
        BRANCH_1.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(A_LEFT_DOUBLE_BRANCH_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_a_left_double_branch() -> Result<ProofWithPublicInputs<F, C, D>> {
        let proof = BRANCH_2.prove(*EMPTY_BRANCH_PROOF, *A_UPDATE_BRANCH_PROOF)?;
        assert_proof(
            &proof,
            STATE_0_DOUBLE_BRANCH_HASHES[ADDRESS_A / 4],
            STATE_1_DOUBLE_BRANCH_HASHES[ADDRESS_A / 4],
            Some((*ADDRESS_A_SUMMARY_HASH, ADDRESS_A as u64 / 4)),
        );
        BRANCH_2.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(BCDE_RIGHT_DOUBLE_BRANCH_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_bcde_right_double_branch() -> Result<ProofWithPublicInputs<F, C, D>> {
        let proof = BRANCH_2.prove(
            *BC_DELETE_CREATE_BRANCH_PROOF,
            *DE_READ_IGNORED_BRANCH_PROOF,
        )?;
        assert_proof(
            &proof,
            STATE_0_DOUBLE_BRANCH_HASHES[ADDRESS_B / 4],
            STATE_1_DOUBLE_BRANCH_HASHES[ADDRESS_B / 4],
            Some((*ADDRESS_BCD_SUMMARY_HASH, ADDRESS_B as u64 / 4)),
        );
        BRANCH_2.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(pub ROOT_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_root() -> Result<ProofWithPublicInputs<F, C, D>> {
        let proof = BRANCH_3.prove(*A_LEFT_DOUBLE_BRANCH_PROOF, *BCDE_RIGHT_DOUBLE_BRANCH_PROOF)?;
        assert_proof(
            &proof,
            *STATE_0_ROOT_HASH,
            *STATE_1_ROOT_HASH,
            Some((*ROOT_SUMMARY_HASH, 0)),
        );
        BRANCH_3.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_root_address() {
        let proof = BRANCH_3
            .prove(*BCDE_RIGHT_DOUBLE_BRANCH_PROOF, *A_LEFT_DOUBLE_BRANCH_PROOF)
            .unwrap();
        BRANCH_3.circuit.verify(proof).unwrap();
    }
}

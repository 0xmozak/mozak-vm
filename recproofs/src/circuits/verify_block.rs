//! Circuits for proving correspondence of all parts of a block

use anyhow::Result;
use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::{HashOut, RichField};
use plonky2::iop::witness::{PartialWitness, WitnessWrite};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::{CircuitConfig, CircuitData};
use plonky2::plonk::config::{AlgebraicHasher, GenericConfig};
use plonky2::plonk::proof::ProofWithPublicInputs;

use super::{match_delta, state_update, verify_tx};

pub mod core;

pub struct Circuit<F, C, const D: usize>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>, {
    /// The tx verifier
    pub tx: core::TxVerifierSubCircuit<D>,

    /// The match delta verifier
    pub match_delta: core::MatchDeltaVerifierSubCircuit<D>,

    /// The state update verifier
    pub state_update: core::StateUpdateVerifierSubCircuit<D>,

    /// The block verifier
    pub block: core::SubCircuit<F, C, D>,

    pub circuit: CircuitData<F, C, D>,
}

impl<F, C, const D: usize> Circuit<F, C, D>
where
    F: RichField + Extendable<D>,
    C: 'static + GenericConfig<D, F = F>,
    <C as GenericConfig<D>>::Hasher: AlgebraicHasher<F>,
{
    #[must_use]
    pub fn new(
        circuit_config: &CircuitConfig,
        tx: &verify_tx::BranchCircuit<F, C, D>,
        md: &match_delta::BranchCircuit<F, C, D>,
        su: &state_update::BranchCircuit<F, C, D>,
    ) -> Self {
        let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());

        let block_inputs = core::SubCircuitInputs::default(&mut builder);

        let tx_targets = core::TxVerifierTargets::build_targets(&mut builder, tx);
        let match_delta_targets = core::MatchDeltaVerifierTargets::build_targets(&mut builder, md);
        let state_update_targets =
            core::StateUpdateVerifierTargets::build_targets(&mut builder, su);
        let block = block_inputs.build(&mut builder);

        builder.connect_hashes(tx_targets.event_root, match_delta_targets.event_root);
        builder.connect(match_delta_targets.block_height, block.inputs.block_height);
        builder.connect_hashes(
            match_delta_targets.state_delta,
            state_update_targets.summary_root,
        );
        builder.connect_hashes(state_update_targets.old_root, block.prev_state_root);
        builder.connect_hashes(state_update_targets.new_root, block.inputs.state_root);

        let circuit = builder.build();

        let public_inputs = &circuit.prover_only.public_inputs;
        let tx = tx_targets.build(public_inputs);
        let match_delta = match_delta_targets.build(public_inputs);
        let state_update = state_update_targets.build(public_inputs);

        Self {
            tx,
            match_delta,
            state_update,
            block,
            circuit,
        }
    }

    pub fn prove_base(
        &self,
        base_state_root: HashOut<F>,
    ) -> Result<ProofWithPublicInputs<F, C, D>> {
        self.block
            .prove_base(&self.circuit.verifier_only, base_state_root)
    }

    pub fn verify_base(&self, base_proof: ProofWithPublicInputs<F, C, D>) -> Result<()> {
        self.block.verify_base(base_proof)
    }

    pub fn prove(
        &self,
        tx_proof: &ProofWithPublicInputs<F, C, D>,
        match_proof: &ProofWithPublicInputs<F, C, D>,
        state_proof: &state_update::BranchProof<F, C, D>,
        prev_proof: &ProofWithPublicInputs<F, C, D>,
    ) -> Result<ProofWithPublicInputs<F, C, D>> {
        let mut inputs = PartialWitness::new();
        self.tx.set_witness(&mut inputs, tx_proof);
        self.match_delta.set_witness(&mut inputs, match_proof);
        self.state_update.set_witness(&mut inputs, state_proof);
        inputs.set_proof_with_pis_target(&self.block.prev_proof, prev_proof);
        self.circuit.prove(inputs)
    }
}

#[cfg(test)]
pub mod test {
    use plonky2::field::types::Field;

    use super::*;
    use crate::circuits::match_delta::test as match_delta;
    use crate::circuits::state_update::test as state_update;
    use crate::circuits::test_data::{STATE_0_ROOT_HASH, STATE_1_ROOT_HASH};
    use crate::circuits::verify_tx::test as verify_tx;
    use crate::test_utils::{C, CONFIG, D, F, NON_ZERO_HASHES, ZERO_HASH};

    #[tested_fixture::tested_fixture(CIRCUIT)]
    fn build_circuit() -> Circuit<F, C, D> {
        Circuit::new(
            &CONFIG,
            *verify_tx::BRANCH,
            *match_delta::BRANCH,
            *state_update::BRANCH_3,
        )
    }

    fn assert_value(
        proof: &ProofWithPublicInputs<F, C, D>,
        base_root: HashOut<F>,
        root: HashOut<F>,
        block_height: i64,
    ) {
        let indices = &CIRCUIT.block.indices;

        let p_base_root = indices.base_state_root.get_any(&proof.public_inputs);
        assert_eq!(p_base_root, base_root.elements);

        let p_root = indices.state_root.get_any(&proof.public_inputs);
        assert_eq!(p_root, root.elements);

        let p_block_height = indices.block_height.get(&proof.public_inputs);
        assert_eq!(p_block_height, F::from_noncanonical_i64(block_height));
    }

    #[test]
    fn verify_zero_base() -> Result<()> {
        let proof = CIRCUIT.prove_base(ZERO_HASH)?;
        assert_value(&proof, ZERO_HASH, ZERO_HASH, 0);
        CIRCUIT.verify_base(proof.clone())?;
        Ok(())
    }

    #[test]
    fn verify_non_zero_base() -> Result<()> {
        let proof = CIRCUIT.prove_base(NON_ZERO_HASHES[0])?;
        assert_value(&proof, NON_ZERO_HASHES[0], NON_ZERO_HASHES[0], 0);
        CIRCUIT.verify_base(proof.clone())?;
        Ok(())
    }

    #[tested_fixture::tested_fixture(STATE_0_BASE_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_state_0_base() -> Result<ProofWithPublicInputs<F, C, D>> {
        let proof = CIRCUIT.prove_base(*STATE_0_ROOT_HASH)?;
        assert_value(&proof, *STATE_0_ROOT_HASH, *STATE_0_ROOT_HASH, 0);
        CIRCUIT.verify_base(proof.clone())?;
        Ok(proof)
    }

    #[test]
    fn verify() -> Result<()> {
        let proof = CIRCUIT.prove(
            *verify_tx::BRANCH_PROOF,
            *match_delta::BRANCH_PROOF,
            *state_update::ROOT_PROOF,
            *STATE_0_BASE_PROOF,
        )?;
        assert_value(&proof, *STATE_0_ROOT_HASH, *STATE_1_ROOT_HASH, 1);
        CIRCUIT.circuit.verify(proof)?;
        Ok(())
    }
}

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
        tx_proof: &ProofWithPublicInputs<F, C, D>,
        match_proof: &ProofWithPublicInputs<F, C, D>,
        state_proof: &ProofWithPublicInputs<F, C, D>,
        base_state_root: HashOut<F>,
        state_root: HashOut<F>,
    ) -> Result<ProofWithPublicInputs<F, C, D>> {
        let mut inputs = PartialWitness::new();
        self.tx.set_witness(&mut inputs, tx_proof);
        self.match_delta.set_witness(&mut inputs, match_proof);
        self.state_update.set_witness(&mut inputs, state_proof);
        self.block.set_base_witness(
            &mut inputs,
            base_state_root,
            state_root,
            &self.circuit.verifier_only,
        );
        self.circuit.prove(inputs)
    }

    pub fn prove(
        &self,
        tx_proof: &ProofWithPublicInputs<F, C, D>,
        match_proof: &ProofWithPublicInputs<F, C, D>,
        state_proof: &ProofWithPublicInputs<F, C, D>,
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

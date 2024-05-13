use anyhow::Result;
use plonky2::field::extension::Extendable;
use plonky2::gates::noop::NoopGate;
use plonky2::hash::hash_types::{HashOut, HashOutTarget, RichField};
use plonky2::iop::target::{BoolTarget, Target};
use plonky2::iop::witness::{PartialWitness, Witness, WitnessWrite};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::{CircuitData, VerifierCircuitTarget, VerifierOnlyCircuitData};
use plonky2::plonk::config::{AlgebraicHasher, GenericConfig};
use plonky2::plonk::proof::{ProofWithPublicInputs, ProofWithPublicInputsTarget};

use crate::circuits::{match_delta, state_update, verify_tx};
use crate::indices::{HashTargetIndex, TargetIndex, VerifierCircuitTargetIndex};
use crate::{circuit_data_for_recursion, dummy_circuit, select_hash, select_verifier};

/// Plonky2's recursion threshold is 2^12 gates.
const RECURSION_THRESHOLD_DEGREE_BITS: usize = 12;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PublicIndices {
    /// The self-recursion verifier
    pub verifier: VerifierCircuitTargetIndex,

    /// The indices of each of the elements of the base state root
    pub base_state_root: HashTargetIndex,

    /// The indices of each of the elements of the state root for this block
    pub state_root: HashTargetIndex,

    /// The index of the block height for this block
    pub block_height: TargetIndex,
}

pub struct SubCircuitInputs {
    /// The recursive verifier
    pub verifier: VerifierCircuitTarget,

    /// The base state root
    pub base_state_root: HashOutTarget,

    /// The state root for this block
    pub state_root: HashOutTarget,

    /// The block height for this block
    pub block_height: Target,
}

pub struct SubCircuit<F, C, const D: usize>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>, {
    /// The dummy circuit
    pub dummy: CircuitData<F, C, D>,

    /// The public inputs
    pub inputs: SubCircuitInputs,

    /// The previous block proof
    pub prev_proof: ProofWithPublicInputsTarget<D>,

    /// The previous state root
    pub prev_state_root: HashOutTarget,

    /// The indicies of the public inputs
    pub indices: PublicIndices,
}

impl SubCircuitInputs {
    pub fn default<F, const D: usize>(builder: &mut CircuitBuilder<F, D>) -> Self
    where
        F: RichField + Extendable<D>, {
        let verifier = builder.add_virtual_verifier_data(builder.config.fri_config.cap_height);
        let base_state_root = builder.add_virtual_hash();
        let state_root = builder.add_virtual_hash();
        let block_height = builder.add_virtual_target();

        let v = Self {
            verifier,
            base_state_root,
            state_root,
            block_height,
        };
        v.register_inputs(builder);
        v
    }

    fn register_inputs<F, const D: usize>(&self, builder: &mut CircuitBuilder<F, D>)
    where
        F: RichField + Extendable<D>, {
        builder.register_public_inputs(&self.verifier.circuit_digest.elements);
        for i in 0..builder.config.fri_config.num_cap_elements() {
            builder.register_public_inputs(&self.verifier.constants_sigmas_cap.0[i].elements);
        }
        builder.register_public_inputs(&self.base_state_root.elements);
        builder.register_public_inputs(&self.state_root.elements);
        builder.register_public_input(self.block_height);
    }

    #[must_use]
    pub fn build<F, C, const D: usize>(
        self,
        builder: &mut CircuitBuilder<F, D>,
    ) -> SubCircuit<F, C, D>
    where
        F: RichField + Extendable<D>,
        C: 'static + GenericConfig<D, F = F>,
        C::Hasher: AlgebraicHasher<F>, {
        let common = circuit_data_for_recursion::<F, C, D>(
            &builder.config,
            RECURSION_THRESHOLD_DEGREE_BITS,
            builder.num_public_inputs(),
        )
        .common;

        let dummy = dummy_circuit::<_, C, D>(&common, |builder| self.register_inputs(builder));

        let prev_proof = builder.add_virtual_proof_with_pis(&common);

        let public_inputs = builder.public_inputs();
        let indices = PublicIndices {
            verifier: VerifierCircuitTargetIndex::new(public_inputs, &self.verifier),
            base_state_root: HashTargetIndex::new(public_inputs, self.base_state_root),
            state_root: HashTargetIndex::new(public_inputs, self.state_root),
            block_height: TargetIndex::new(public_inputs, self.block_height),
        };
        let prev_block_height = indices.block_height.get(&prev_proof.public_inputs);

        let non_base = builder.is_nonzero(prev_block_height);

        // Connect previous verifier data to current one. This guarantees that every
        // proof in the cycle uses the same verifier data.
        let prev_verifier = indices.verifier.get(&prev_proof.public_inputs);
        builder.connect_verifier_data(&self.verifier, &prev_verifier);

        let dummy_verifier = builder.constant_verifier_data(&dummy.verifier_only);
        let verifier_calc = select_verifier(builder, non_base, &self.verifier, &dummy_verifier);
        builder.verify_proof::<C>(&prev_proof, &verifier_calc, &common);

        // Connect heights
        let block_height_calc = builder.add_const(prev_block_height, F::ONE);
        builder.connect(self.block_height, block_height_calc);

        // Ensure base states match
        let prev_base_state = indices.base_state_root.get(&prev_proof.public_inputs);
        builder.connect_hashes(self.base_state_root, prev_base_state);

        let prev_state_root = indices.state_root.get(&prev_proof.public_inputs);

        // Ensure base state is actually the base
        let prev_base_state_calc = select_hash(builder, non_base, prev_base_state, prev_state_root);
        builder.connect_hashes(prev_base_state_calc, prev_base_state);

        // Make sure we have enough gates to match `common_data`.
        while builder.num_gates() < (common.degree() / 2) {
            builder.add_gate(NoopGate, vec![]);
        }
        // Make sure we have every gate to match `common_data`.
        for g in &common.gates {
            builder.add_gate_to_gate_set(g.clone());
        }

        SubCircuit {
            dummy,
            inputs: self,
            prev_proof,
            prev_state_root,
            indices,
        }
    }
}

impl<F, C, const D: usize> SubCircuit<F, C, D>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    <C as GenericConfig<D>>::Hasher: AlgebraicHasher<F>,
{
    pub fn prove_base(
        &self,
        verifier: &VerifierOnlyCircuitData<C, D>,
        base_state_root: HashOut<F>,
    ) -> Result<ProofWithPublicInputs<F, C, D>> {
        self.prove_base_unsafe(verifier, base_state_root, base_state_root, 0)
    }

    pub fn prove_base_unsafe(
        &self,
        verifier: &VerifierOnlyCircuitData<C, D>,
        base_state_root: HashOut<F>,
        state_root: HashOut<F>,
        block_height: u64,
    ) -> Result<ProofWithPublicInputs<F, C, D>> {
        let mut dummy_inputs = PartialWitness::new();

        // Set the base inputs
        dummy_inputs.set_verifier_data_target(&self.inputs.verifier, verifier);
        dummy_inputs.set_hash_target(self.inputs.base_state_root, base_state_root);
        dummy_inputs.set_hash_target(self.inputs.state_root, state_root);
        dummy_inputs.set_target(
            self.inputs.block_height,
            F::from_canonical_u64(block_height),
        );

        // Zero out all other inputs
        for i in 0..self.dummy.common.num_public_inputs {
            let target = self.dummy.prover_only.public_inputs[i];
            if dummy_inputs.try_get_target(target).is_none() {
                dummy_inputs.set_target(target, F::ZERO);
            }
        }

        self.dummy.prove(dummy_inputs)
    }

    pub fn verify_base(&self, base_proof: ProofWithPublicInputs<F, C, D>) -> Result<()> {
        self.dummy.verify(base_proof)
    }

    pub fn set_witness(
        &self,
        inputs: &mut PartialWitness<F>,
        state_root: HashOut<F>,
        prev_proof: &ProofWithPublicInputs<F, C, D>,
    ) {
        inputs.set_hash_target(self.inputs.state_root, state_root);
        inputs.set_proof_with_pis_target(&self.prev_proof, prev_proof);
    }

    pub fn set_witness_unsafe(
        &self,
        inputs: &mut PartialWitness<F>,
        verifier: &VerifierOnlyCircuitData<C, D>,
        base_state_root: HashOut<F>,
        state_root: HashOut<F>,
        block_height: u64,
        prev_proof: &ProofWithPublicInputs<F, C, D>,
    ) {
        inputs.set_verifier_data_target(&self.inputs.verifier, verifier);
        inputs.set_hash_target(self.inputs.base_state_root, base_state_root);
        inputs.set_hash_target(self.inputs.state_root, state_root);
        inputs.set_target(
            self.inputs.block_height,
            F::from_canonical_u64(block_height),
        );
        inputs.set_proof_with_pis_target(&self.prev_proof, prev_proof);
    }
}

pub struct TxVerifierTargets<const D: usize> {
    /// The tx proof
    pub proof: ProofWithPublicInputsTarget<D>,

    /// The presence flag for the event root
    pub events_present: BoolTarget,

    /// The event root
    pub event_root: HashOutTarget,
}

pub struct TxVerifierSubCircuit<const D: usize> {
    pub targets: TxVerifierTargets<D>,
}

impl<const D: usize> TxVerifierTargets<D> {
    #[must_use]
    pub fn build_targets<F, C>(
        builder: &mut CircuitBuilder<F, D>,
        tx: &verify_tx::BranchCircuit<F, C, D>,
    ) -> Self
    where
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F>,
        <C as GenericConfig<D>>::Hasher: AlgebraicHasher<F>, {
        let circuit = &tx.circuit;
        let proof = builder.add_virtual_proof_with_pis(&circuit.common);
        let verifier = builder.constant_verifier_data(&circuit.verifier_only);
        builder.verify_proof::<C>(&proof, &verifier, &circuit.common);

        let events_present = tx.events.indices.hash_present.get(&proof.public_inputs);
        let event_root = tx.events.indices.hash.get(&proof.public_inputs);

        Self {
            proof,
            events_present,
            event_root,
        }
    }
}

impl<const D: usize> TxVerifierTargets<D> {
    #[must_use]
    pub fn build(self, _public_inputs: &[Target]) -> TxVerifierSubCircuit<D> {
        TxVerifierSubCircuit { targets: self }
    }
}

impl<const D: usize> TxVerifierSubCircuit<D> {
    pub fn set_witness<F, C>(
        &self,
        inputs: &mut PartialWitness<F>,
        tx_proof: &ProofWithPublicInputs<F, C, D>,
    ) where
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F>,
        <C as GenericConfig<D>>::Hasher: AlgebraicHasher<F>, {
        inputs.set_proof_with_pis_target(&self.targets.proof, tx_proof);
    }
}

pub struct MatchDeltaVerifierTargets<const D: usize> {
    /// The match delta proof
    pub proof: ProofWithPublicInputsTarget<D>,

    /// The event root
    pub event_root: HashOutTarget,

    /// The block height
    pub block_height: Target,

    /// The state delta root
    pub state_delta: HashOutTarget,
}

pub struct MatchDeltaVerifierSubCircuit<const D: usize> {
    pub targets: MatchDeltaVerifierTargets<D>,
}

impl<const D: usize> MatchDeltaVerifierTargets<D> {
    #[must_use]
    pub fn build_targets<F, C>(
        builder: &mut CircuitBuilder<F, D>,
        md: &match_delta::BranchCircuit<F, C, D>,
    ) -> Self
    where
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F>,
        <C as GenericConfig<D>>::Hasher: AlgebraicHasher<F>, {
        let circuit = &md.circuit;
        let proof = builder.add_virtual_proof_with_pis(&circuit.common);
        let verifier = builder.constant_verifier_data(&circuit.verifier_only);
        builder.verify_proof::<C>(&proof, &verifier, &circuit.common);

        let event_root = md
            .event_hash
            .indices
            .unpruned_hash
            .get(&proof.public_inputs);
        let block_height = md.block_height.indices.values.get(&proof.public_inputs)[0];
        let state_delta = md
            .state_hash
            .indices
            .unpruned_hash
            .get(&proof.public_inputs);

        Self {
            proof,
            event_root,
            block_height,
            state_delta,
        }
    }
}

impl<const D: usize> MatchDeltaVerifierTargets<D> {
    #[must_use]
    pub fn build(self, _public_inputs: &[Target]) -> MatchDeltaVerifierSubCircuit<D> {
        MatchDeltaVerifierSubCircuit { targets: self }
    }
}

impl<const D: usize> MatchDeltaVerifierSubCircuit<D> {
    pub fn set_witness<F, C>(
        &self,
        inputs: &mut PartialWitness<F>,
        match_proof: &ProofWithPublicInputs<F, C, D>,
    ) where
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F>,
        <C as GenericConfig<D>>::Hasher: AlgebraicHasher<F>, {
        inputs.set_proof_with_pis_target(&self.targets.proof, match_proof);
    }
}

pub struct StateUpdateVerifierTargets<const D: usize> {
    /// The state update proof
    pub proof: ProofWithPublicInputsTarget<D>,

    /// The summarized root
    pub summary_root: HashOutTarget,

    /// The old state root
    pub old_root: HashOutTarget,

    /// The new state root
    pub new_root: HashOutTarget,
}

pub struct StateUpdateVerifierSubCircuit<const D: usize> {
    pub targets: StateUpdateVerifierTargets<D>,
}

impl<const D: usize> StateUpdateVerifierTargets<D> {
    #[must_use]
    pub fn build_targets<F, C>(
        builder: &mut CircuitBuilder<F, D>,
        su: &state_update::BranchCircuit<F, C, D>,
    ) -> Self
    where
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F>,
        <C as GenericConfig<D>>::Hasher: AlgebraicHasher<F>, {
        let circuit = &su.circuit;
        let proof = builder.add_virtual_proof_with_pis(&circuit.common);
        let verifier = builder.constant_verifier_data(&circuit.verifier_only);
        builder.verify_proof::<C>(&proof, &verifier, &circuit.common);

        let summary_root = su.summarized.indices.summary_hash.get(&proof.public_inputs);
        let old_root = su.old.indices.unpruned_hash.get(&proof.public_inputs);
        let new_root = su.new.indices.unpruned_hash.get(&proof.public_inputs);

        let _true = builder._true();
        let summary_present = su
            .summarized
            .indices
            .summary_hash_present
            .get(&proof.public_inputs);
        builder.connect(summary_present.target, _true.target);

        let zero = builder.zero();
        let address = su.address.indices.node_address.get(&proof.public_inputs);
        builder.connect(address, zero);

        Self {
            proof,
            summary_root,
            old_root,
            new_root,
        }
    }
}

impl<const D: usize> StateUpdateVerifierTargets<D> {
    #[must_use]
    pub fn build(self, _public_inputs: &[Target]) -> StateUpdateVerifierSubCircuit<D> {
        StateUpdateVerifierSubCircuit { targets: self }
    }
}

impl<const D: usize> StateUpdateVerifierSubCircuit<D> {
    pub fn set_witness<F, C>(
        &self,
        inputs: &mut PartialWitness<F>,
        match_proof: &ProofWithPublicInputs<F, C, D>,
    ) where
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F>,
        <C as GenericConfig<D>>::Hasher: AlgebraicHasher<F>, {
        inputs.set_proof_with_pis_target(&self.targets.proof, match_proof);
    }
}

#[cfg(test)]
mod test {
    use std::panic::catch_unwind;

    use plonky2::field::types::Field;
    use plonky2::plonk::circuit_data::CircuitConfig;

    use super::*;
    use crate::test_utils::{C, CONFIG, D, F, NON_ZERO_HASHES, ZERO_HASH};

    pub struct DummyCircuit {
        pub verify_block: SubCircuit<F, C, D>,
        pub circuit: CircuitData<F, C, D>,
    }

    impl DummyCircuit {
        #[must_use]
        pub fn new(circuit_config: &CircuitConfig) -> Self {
            let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());

            let verify_block_inputs = SubCircuitInputs::default(&mut builder);

            let verify_block = verify_block_inputs.build(&mut builder);

            let circuit = builder.build();

            let public_inputs = &circuit.prover_only.public_inputs;
            let indices = PublicIndices {
                verifier: VerifierCircuitTargetIndex::new(
                    public_inputs,
                    &verify_block.inputs.verifier,
                ),
                base_state_root: HashTargetIndex::new(
                    public_inputs,
                    verify_block.inputs.base_state_root,
                ),
                state_root: HashTargetIndex::new(public_inputs, verify_block.inputs.state_root),
                block_height: TargetIndex::new(public_inputs, verify_block.inputs.block_height),
            };
            assert_eq!(indices, verify_block.indices);

            Self {
                verify_block,
                circuit,
            }
        }

        pub fn prove_base(
            &self,
            base_state_root: HashOut<F>,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            self.verify_block
                .prove_base(&self.circuit.verifier_only, base_state_root)
        }

        pub fn prove_base_unsafe(
            &self,
            verifier: Option<&VerifierOnlyCircuitData<C, D>>,
            base_state_root: HashOut<F>,
            state_root: HashOut<F>,
            block_height: u64,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            self.verify_block.prove_base_unsafe(
                verifier.unwrap_or(&self.circuit.verifier_only),
                base_state_root,
                state_root,
                block_height,
            )
        }

        pub fn prove(
            &self,
            state_root: HashOut<F>,
            prev_proof: &ProofWithPublicInputs<F, C, D>,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.verify_block
                .set_witness(&mut inputs, state_root, prev_proof);
            self.circuit.prove(inputs)
        }

        pub fn prove_unsafe(
            &self,
            verifier: Option<&VerifierOnlyCircuitData<C, D>>,
            base_state_root: HashOut<F>,
            state_root: HashOut<F>,
            block_height: u64,
            prev_proof: &ProofWithPublicInputs<F, C, D>,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.verify_block.set_witness_unsafe(
                &mut inputs,
                verifier.unwrap_or(&self.circuit.verifier_only),
                base_state_root,
                state_root,
                block_height,
                prev_proof,
            );
            self.circuit.prove(inputs)
        }

        pub fn verify_base(&self, base_proof: ProofWithPublicInputs<F, C, D>) -> Result<()> {
            self.verify_block.verify_base(base_proof)
        }
    }

    #[tested_fixture::tested_fixture(CIRCUIT)]
    fn build_circuit() -> DummyCircuit { DummyCircuit::new(&CONFIG) }

    fn assert_proof(
        proof: &ProofWithPublicInputs<F, C, D>,
        base_root: HashOut<F>,
        root: HashOut<F>,
        block_height: u64,
    ) {
        let indices = &CIRCUIT.verify_block.indices;

        let p_base_root = indices.base_state_root.get_any(&proof.public_inputs);
        assert_eq!(p_base_root, base_root.elements);

        let p_root = indices.state_root.get_any(&proof.public_inputs);
        assert_eq!(p_root, root.elements);

        let p_block_height = indices.block_height.get(&proof.public_inputs);
        assert_eq!(p_block_height, F::from_canonical_u64(block_height));
    }

    #[tested_fixture::tested_fixture(ZERO_BASE_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_zero_base() -> Result<ProofWithPublicInputs<F, C, D>> {
        let proof = CIRCUIT.prove_base(ZERO_HASH)?;
        CIRCUIT.verify_base(proof.clone())?;
        assert_proof(&proof, ZERO_HASH, ZERO_HASH, 0);
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(NON_ZERO_BASE_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_non_zero_base() -> Result<ProofWithPublicInputs<F, C, D>> {
        let proof = CIRCUIT.prove_base(NON_ZERO_HASHES[0])?;
        CIRCUIT.verify_base(proof.clone())?;
        assert_proof(&proof, NON_ZERO_HASHES[0], NON_ZERO_HASHES[0], 0);
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(BAD_HEIGHT_ZERO_BASE_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_bad_height_zero_base() -> Result<ProofWithPublicInputs<F, C, D>> {
        let proof = CIRCUIT.prove_base_unsafe(None, ZERO_HASH, ZERO_HASH, 1)?;
        CIRCUIT.verify_base(proof.clone())?;
        assert_proof(&proof, ZERO_HASH, ZERO_HASH, 1);
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(BAD_VERIFIER_ZERO_BASE_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_bad_verifier_zero_base() -> Result<ProofWithPublicInputs<F, C, D>> {
        let proof = CIRCUIT.prove_base_unsafe(
            Some(&CIRCUIT.verify_block.dummy.verifier_only),
            ZERO_HASH,
            ZERO_HASH,
            0,
        )?;
        CIRCUIT.verify_base(proof.clone())?;
        assert_proof(&proof, ZERO_HASH, ZERO_HASH, 0);
        Ok(proof)
    }

    #[test]
    fn verify_static_zero() -> Result<()> {
        let mut proof = ZERO_BASE_PROOF.clone();
        for i in 1..5 {
            proof = CIRCUIT.prove(ZERO_HASH, &proof)?;
            assert_proof(&proof, ZERO_HASH, ZERO_HASH, i);
            CIRCUIT.circuit.verify(proof.clone())?;
        }
        Ok(())
    }

    #[test]
    fn verify_static_nonzero() -> Result<()> {
        let mut proof = NON_ZERO_BASE_PROOF.clone();
        for i in 1..5 {
            proof = CIRCUIT.prove(NON_ZERO_HASHES[0], &proof)?;
            assert_proof(&proof, NON_ZERO_HASHES[0], NON_ZERO_HASHES[0], i);
            CIRCUIT.circuit.verify(proof.clone())?;
        }
        Ok(())
    }

    #[test]
    fn verify_zero_based() -> Result<()> {
        let mut proof = ZERO_BASE_PROOF.clone();
        for (i, hash) in NON_ZERO_HASHES.into_iter().enumerate() {
            proof = CIRCUIT.prove(hash, &proof)?;
            assert_proof(&proof, ZERO_HASH, hash, i as u64 + 1);
            CIRCUIT.circuit.verify(proof.clone())?;
        }
        Ok(())
    }

    #[test]
    fn verify_nonzero_based() -> Result<()> {
        let mut proof = NON_ZERO_BASE_PROOF.clone();
        for (i, hash) in NON_ZERO_HASHES.into_iter().enumerate() {
            proof = CIRCUIT.prove(hash, &proof)?;
            assert_proof(&proof, NON_ZERO_HASHES[0], hash, i as u64 + 1);
            CIRCUIT.circuit.verify(proof.clone())?;
        }
        Ok(())
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_base() {
        let proof = CIRCUIT
            .prove_unsafe(None, NON_ZERO_HASHES[0], ZERO_HASH, 0, *ZERO_BASE_PROOF)
            .unwrap();
        CIRCUIT.circuit.verify(proof.clone()).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_height_0() {
        let proof = CIRCUIT
            .prove_unsafe(None, ZERO_HASH, ZERO_HASH, 0, *ZERO_BASE_PROOF)
            .unwrap();
        CIRCUIT.circuit.verify(proof.clone()).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_height_2() {
        let proof = CIRCUIT
            .prove_unsafe(None, ZERO_HASH, ZERO_HASH, 2, *ZERO_BASE_PROOF)
            .unwrap();
        CIRCUIT.circuit.verify(proof.clone()).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_height_1() {
        let proof = catch_unwind(|| CIRCUIT.prove(ZERO_HASH, *ZERO_BASE_PROOF).unwrap())
            .expect("shouldn't fail");

        let proof = CIRCUIT
            .prove_unsafe(None, ZERO_HASH, NON_ZERO_HASHES[0], 1, &proof)
            .unwrap();
        CIRCUIT.circuit.verify(proof.clone()).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_base_height_0() {
        let proof = CIRCUIT
            .prove_unsafe(None, ZERO_HASH, ZERO_HASH, 0, &BAD_HEIGHT_ZERO_BASE_PROOF)
            .unwrap();
        CIRCUIT.circuit.verify(proof.clone()).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_base_height_1() {
        let proof = CIRCUIT
            .prove_unsafe(None, ZERO_HASH, ZERO_HASH, 1, &BAD_HEIGHT_ZERO_BASE_PROOF)
            .unwrap();
        CIRCUIT.circuit.verify(proof.clone()).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_base_height_2() {
        let proof = CIRCUIT
            .prove_unsafe(None, ZERO_HASH, ZERO_HASH, 2, &BAD_HEIGHT_ZERO_BASE_PROOF)
            .unwrap();
        CIRCUIT.circuit.verify(proof.clone()).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_base_verifier_match() {
        let proof = CIRCUIT
            .prove_unsafe(None, ZERO_HASH, ZERO_HASH, 1, &BAD_VERIFIER_ZERO_BASE_PROOF)
            .unwrap();
        CIRCUIT.circuit.verify(proof.clone()).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_base_verifier_mismatch() {
        let proof = CIRCUIT
            .prove_unsafe(
                Some(&CIRCUIT.circuit.verifier_only),
                ZERO_HASH,
                ZERO_HASH,
                1,
                &BAD_VERIFIER_ZERO_BASE_PROOF,
            )
            .unwrap();
        CIRCUIT.circuit.verify(proof.clone()).unwrap();
    }
}

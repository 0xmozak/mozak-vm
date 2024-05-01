use std::iter::zip;

use plonky2::field::extension::Extendable;
use plonky2::gates::noop::NoopGate;
use plonky2::hash::hash_types::{
    HashOut, HashOutTarget, MerkleCapTarget, RichField, NUM_HASH_OUT_ELTS,
};
use plonky2::iop::target::{BoolTarget, Target};
use plonky2::iop::witness::{PartialWitness, Witness, WitnessWrite};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::{CircuitData, VerifierCircuitTarget, VerifierOnlyCircuitData};
use plonky2::plonk::config::{AlgebraicHasher, GenericConfig};
use plonky2::plonk::proof::{ProofWithPublicInputs, ProofWithPublicInputsTarget};

use crate::circuits::{match_delta, state_update, verify_tx};
use crate::{
    circuit_data_for_recursion, dummy_circuit, find_hash, find_target, select_hash, select_verifier,
};

/// Plonky2's recursion threshold is 2^12 gates. We use a slightly relaxed
/// threshold here to support the case that two proofs are verified in the same
/// circuit.
const RECURSION_THRESHOLD_DEGREE_BITS: usize = 12;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PublicIndices {
    /// A digest of the "circuit" (i.e. the instance, minus public inputs),
    /// which can be used to seed Fiat-Shamir.
    pub circuit_digest: [usize; NUM_HASH_OUT_ELTS],

    /// A commitment to each constant polynomial and each permutation
    /// polynomial.
    pub constants_sigmas_cap: Vec<[usize; NUM_HASH_OUT_ELTS]>,

    /// The indices of each of the elements of the base state root
    pub base_state_root: [usize; NUM_HASH_OUT_ELTS],

    /// The indices of each of the elements of the state root for this block
    pub state_root: [usize; NUM_HASH_OUT_ELTS],

    /// The index of the block height for this block
    pub block_height: usize,
}

impl PublicIndices {
    /// Extract `circuit_digest` from an array of public inputs.
    pub fn get_circuit_digest<T: Copy>(&self, public_inputs: &[T]) -> [T; NUM_HASH_OUT_ELTS] {
        self.circuit_digest.map(|i| public_inputs[i])
    }

    /// Insert `circuit_digest` into an array of public inputs.
    pub fn set_circuit_digest<T>(&self, public_inputs: &mut [T], v: [T; NUM_HASH_OUT_ELTS]) {
        for (i, v) in v.into_iter().enumerate() {
            public_inputs[self.circuit_digest[i]] = v;
        }
    }

    /// Extract `constants_sigmas_cap` from an array of public inputs.
    pub fn get_constants_sigmas_cap<T: Copy>(
        &self,
        public_inputs: &[T],
    ) -> Vec<[T; NUM_HASH_OUT_ELTS]> {
        self.constants_sigmas_cap
            .iter()
            .map(|v| v.map(|i| public_inputs[i]))
            .collect()
    }

    /// Insert `constants_sigmas_cap` into an array of public inputs.
    pub fn set_constants_sigmas_cap<T>(
        &self,
        public_inputs: &mut [T],
        vs: Vec<[T; NUM_HASH_OUT_ELTS]>,
    ) {
        for (i, v) in vs.into_iter().enumerate() {
            for (i, v) in zip(self.constants_sigmas_cap[i], v) {
                public_inputs[i] = v;
            }
        }
    }

    /// Extract `base_state_root` from an array of public inputs.
    pub fn get_base_state_root<T: Copy>(&self, public_inputs: &[T]) -> [T; NUM_HASH_OUT_ELTS] {
        self.base_state_root.map(|i| public_inputs[i])
    }

    /// Insert `base_state_root` into an array of public inputs.
    pub fn set_base_state_root<T>(&self, public_inputs: &mut [T], v: [T; NUM_HASH_OUT_ELTS]) {
        for (i, v) in v.into_iter().enumerate() {
            public_inputs[self.base_state_root[i]] = v;
        }
    }

    /// Extract `state_root` from an array of public inputs.
    pub fn get_state_root<T: Copy>(&self, public_inputs: &[T]) -> [T; NUM_HASH_OUT_ELTS] {
        self.state_root.map(|i| public_inputs[i])
    }

    /// Insert `state_root` into an array of public inputs.
    pub fn set_state_root<T>(&self, public_inputs: &mut [T], v: [T; NUM_HASH_OUT_ELTS]) {
        for (i, v) in v.into_iter().enumerate() {
            public_inputs[self.state_root[i]] = v;
        }
    }

    /// Extract `block_height` from an array of public inputs.
    pub fn get_block_height<T: Copy>(&self, public_inputs: &[T]) -> T {
        public_inputs[self.block_height]
    }

    /// Insert `block_height` into an array of public inputs.
    pub fn set_block_height<T>(&self, public_inputs: &mut [T], v: T) {
        public_inputs[self.block_height] = v;
    }
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
            circuit_digest: find_hash(public_inputs, self.verifier.circuit_digest),
            constants_sigmas_cap: self
                .verifier
                .constants_sigmas_cap
                .0
                .iter()
                .map(|v| find_hash(public_inputs, *v))
                .collect(),
            base_state_root: find_hash(public_inputs, self.base_state_root),
            state_root: find_hash(public_inputs, self.state_root),
            block_height: find_target(public_inputs, self.block_height),
        };

        let non_base = builder.is_nonzero(self.block_height);

        // Connect previous verifier data to current one. This guarantees that every
        // proof in the cycle uses the same verifier data.
        let prev_verifier = VerifierCircuitTarget {
            circuit_digest: indices.get_circuit_digest(&prev_proof.public_inputs).into(),
            constants_sigmas_cap: MerkleCapTarget(
                indices
                    .get_constants_sigmas_cap(&prev_proof.public_inputs)
                    .into_iter()
                    .map(Into::into)
                    .collect(),
            ),
        };
        builder.connect_verifier_data(&self.verifier, &prev_verifier);

        let dummy_verifier = builder.constant_verifier_data(&dummy.verifier_only);
        let verifier_calc = select_verifier(builder, non_base, &self.verifier, &dummy_verifier);
        builder.verify_proof::<C>(&prev_proof, &verifier_calc, &common);

        // Connect heights
        let block_height_calc =
            builder.add_const(indices.get_block_height(&prev_proof.public_inputs), F::ONE);
        builder.connect(self.block_height, block_height_calc);

        // Ensure base states match
        let prev_base_state = HashOutTarget {
            elements: indices.get_base_state_root(&prev_proof.public_inputs),
        };
        builder.connect_hashes(self.base_state_root, prev_base_state);

        let prev_state_root = HashOutTarget {
            elements: indices.get_state_root(&prev_proof.public_inputs),
        };

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
    pub fn set_base_witness(
        &self,
        inputs: &mut PartialWitness<F>,
        base_state_root: HashOut<F>,
        state_root: HashOut<F>,
        verifier: &VerifierOnlyCircuitData<C, D>,
    ) {
        let dummy_proof = {
            let mut dummy_inputs = PartialWitness::new();
            dummy_inputs.set_verifier_data_target(&self.inputs.verifier, verifier);
            dummy_inputs.set_hash_target(self.inputs.base_state_root, base_state_root);
            dummy_inputs.set_hash_target(self.inputs.state_root, base_state_root);
            dummy_inputs.set_target(self.inputs.block_height, F::NEG_ONE);
            for i in 0..self.dummy.common.num_public_inputs {
                let target = self.dummy.prover_only.public_inputs[i];
                if dummy_inputs.try_get_target(target).is_none() {
                    dummy_inputs.set_target(target, F::ZERO);
                }
            }
            self.dummy.prove(dummy_inputs).unwrap()
        };

        inputs.set_verifier_data_target(&self.inputs.verifier, verifier);
        inputs.set_hash_target(self.inputs.base_state_root, base_state_root);
        inputs.set_hash_target(self.inputs.state_root, state_root);
        inputs.set_target(self.inputs.block_height, F::ZERO);
        inputs.set_proof_with_pis_target(&self.prev_proof, &dummy_proof);
    }

    pub fn set_witness(
        &self,
        inputs: &mut PartialWitness<F>,
        base_state_root: Option<HashOut<F>>,
        state_root: HashOut<F>,
        block_height: Option<u64>,
        prev_proof: &ProofWithPublicInputs<F, C, D>,
    ) {
        if let Some(base_state_root) = base_state_root {
            inputs.set_hash_target(self.inputs.base_state_root, base_state_root);
        }
        inputs.set_hash_target(self.inputs.state_root, state_root);
        if let Some(block_height) = block_height {
            inputs.set_target(
                self.inputs.block_height,
                F::from_canonical_u64(block_height),
            );
        }
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

        let events_present =
            BoolTarget::new_unsafe(tx.events.indices.get_hash_present(&proof.public_inputs));
        let event_root = HashOutTarget {
            elements: tx.events.indices.get_hash(&proof.public_inputs),
        };

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

        let event_root = HashOutTarget {
            elements: md
                .event_hash
                .indices
                .get_unpruned_hash(&proof.public_inputs),
        };
        let block_height = md.block_height.indices.get_values(&proof.public_inputs)[0];
        let state_delta = HashOutTarget {
            elements: md
                .state_hash
                .indices
                .get_unpruned_hash(&proof.public_inputs),
        };

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

        let summary_root = HashOutTarget {
            elements: su.summarized.indices.get_summary_hash(&proof.public_inputs),
        };
        let old_root = HashOutTarget {
            elements: su.old.indices.get_unpruned_hash(&proof.public_inputs),
        };
        let new_root = HashOutTarget {
            elements: su.new.indices.get_unpruned_hash(&proof.public_inputs),
        };

        let _true = builder._true();
        let summary_present = su
            .summarized
            .indices
            .get_summary_hash_present(&proof.public_inputs);
        builder.connect(summary_present, _true.target);

        let zero = builder.zero();
        let address = su.address.indices.get_node_address(&proof.public_inputs);
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

    use anyhow::Result;
    use lazy_static::lazy_static;
    use plonky2::field::types::Field;
    use plonky2::plonk::circuit_data::{CircuitConfig, CircuitData};

    use super::*;
    use crate::test_utils::{hash_str, C, CONFIG, D, F};

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
                circuit_digest: find_hash(
                    public_inputs,
                    verify_block.inputs.verifier.circuit_digest,
                ),
                constants_sigmas_cap: verify_block
                    .inputs
                    .verifier
                    .constants_sigmas_cap
                    .0
                    .iter()
                    .map(|v| find_hash(public_inputs, *v))
                    .collect(),
                base_state_root: find_hash(public_inputs, verify_block.inputs.base_state_root),
                state_root: find_hash(public_inputs, verify_block.inputs.state_root),
                block_height: find_target(public_inputs, verify_block.inputs.block_height),
            };
            assert_eq!(indices, verify_block.indices);

            Self {
                verify_block,
                circuit,
            }
        }

        pub fn prove(
            &self,
            state_root: HashOut<F>,
            prev_proof: Result<&ProofWithPublicInputs<F, C, D>, HashOut<F>>,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            match prev_proof {
                Ok(prev_proof) => {
                    self.verify_block
                        .set_witness(&mut inputs, None, state_root, None, prev_proof);
                }
                Err(base_state_root) => {
                    self.verify_block.set_base_witness(
                        &mut inputs,
                        base_state_root,
                        state_root,
                        &self.circuit.verifier_only,
                    );
                }
            }
            self.circuit.prove(inputs)
        }

        pub fn prove_unsafe(
            &self,
            base_state_root: Option<HashOut<F>>,
            state_root: HashOut<F>,
            block_height: Option<u64>,
            prev_proof: &ProofWithPublicInputs<F, C, D>,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.verify_block.set_witness(
                &mut inputs,
                base_state_root,
                state_root,
                block_height,
                prev_proof,
            );
            self.circuit.prove(inputs)
        }
    }

    lazy_static! {
        static ref CIRCUIT: DummyCircuit = DummyCircuit::new(&CONFIG);
    }

    #[test]
    fn verify_static_zero() -> Result<()> {
        let zero_hash = HashOut::from([F::ZERO; 4]);

        let mut proof = CIRCUIT.prove(zero_hash, Err(zero_hash))?;
        CIRCUIT.circuit.verify(proof.clone())?;

        for _ in 0..4 {
            proof = CIRCUIT.prove(zero_hash, Ok(&proof))?;
            CIRCUIT.circuit.verify(proof.clone())?;
        }

        Ok(())
    }

    #[test]
    fn verify_static_nonzero() -> Result<()> {
        let non_zero_hash = hash_str("Non-Zero Hash 1");

        let mut proof = CIRCUIT.prove(non_zero_hash, Err(non_zero_hash))?;
        CIRCUIT.circuit.verify(proof.clone())?;

        for _ in 0..4 {
            proof = CIRCUIT.prove(non_zero_hash, Ok(&proof))?;
            CIRCUIT.circuit.verify(proof.clone())?;
        }

        Ok(())
    }

    #[test]
    fn verify_zero_based() -> Result<()> {
        let zero_hash = HashOut::from([F::ZERO; 4]);
        let non_zero_hash = hash_str("Non-Zero Hash 0");
        let hashes = [
            hash_str("Non-Zero Hash 1"),
            hash_str("Non-Zero Hash 2"),
            hash_str("Non-Zero Hash 3"),
            hash_str("Non-Zero Hash 4"),
        ];

        let mut proof = CIRCUIT.prove(non_zero_hash, Err(zero_hash))?;
        CIRCUIT.circuit.verify(proof.clone())?;

        for hash in hashes {
            proof = CIRCUIT.prove(hash, Ok(&proof))?;
            CIRCUIT.circuit.verify(proof.clone())?;
        }

        Ok(())
    }

    #[test]
    fn verify_nonzero_based() -> Result<()> {
        let non_zero_hash_0 = hash_str("Non-Zero Hash 0");
        let non_zero_hash_1 = hash_str("Non-Zero Hash 0");
        let hashes = [
            hash_str("Non-Zero Hash 2"),
            hash_str("Non-Zero Hash 3"),
            hash_str("Non-Zero Hash 4"),
        ];

        let mut proof = CIRCUIT.prove(non_zero_hash_1, Err(non_zero_hash_0))?;
        CIRCUIT.circuit.verify(proof.clone())?;

        for hash in hashes {
            proof = CIRCUIT.prove(hash, Ok(&proof))?;
            CIRCUIT.circuit.verify(proof.clone())?;
        }

        Ok(())
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_base() {
        let non_zero_hash_1 = hash_str("Non-Zero Hash 1");
        let non_zero_hash_2 = hash_str("Non-Zero Hash 2");

        let proof = catch_unwind(|| {
            let zero_hash = HashOut::from([F::ZERO; 4]);
            let proof = CIRCUIT.prove(zero_hash, Err(zero_hash))?;
            CIRCUIT.circuit.verify(proof.clone())?;
            Result::<_>::Ok(proof)
        })
        .expect("shouldn't fail")
        .unwrap();

        let proof = CIRCUIT
            .prove_unsafe(Some(non_zero_hash_1), non_zero_hash_2, None, &proof)
            .unwrap();
        CIRCUIT.circuit.verify(proof.clone()).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_height_0() {
        let non_zero_hash_1 = hash_str("Non-Zero Hash 1");

        let proof = catch_unwind(|| {
            let zero_hash = HashOut::from([F::ZERO; 4]);
            let proof = CIRCUIT.prove(zero_hash, Err(zero_hash))?;
            CIRCUIT.circuit.verify(proof.clone())?;
            Result::<_>::Ok(proof)
        })
        .expect("shouldn't fail")
        .unwrap();

        let proof = CIRCUIT
            .prove_unsafe(None, non_zero_hash_1, Some(0), &proof)
            .unwrap();
        CIRCUIT.circuit.verify(proof.clone()).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_height_2() {
        let non_zero_hash_1 = hash_str("Non-Zero Hash 1");

        let proof = catch_unwind(|| {
            let zero_hash = HashOut::from([F::ZERO; 4]);
            let proof = CIRCUIT.prove(zero_hash, Err(zero_hash))?;
            CIRCUIT.circuit.verify(proof.clone())?;
            Result::<_>::Ok(proof)
        })
        .expect("shouldn't fail")
        .unwrap();

        let proof = CIRCUIT
            .prove_unsafe(None, non_zero_hash_1, Some(2), &proof)
            .unwrap();
        CIRCUIT.circuit.verify(proof.clone()).unwrap();
    }
}

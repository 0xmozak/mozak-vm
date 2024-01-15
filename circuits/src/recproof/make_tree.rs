use anyhow::Result;
use iter_fixed::IntoIteratorFixed;
use plonky2::field::extension::Extendable;
use plonky2::gates::noop::NoopGate;
use plonky2::hash::hash_types::{HashOut, HashOutTarget, RichField, NUM_HASH_OUT_ELTS};
use plonky2::iop::witness::{PartialWitness, WitnessWrite};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::{
    CircuitConfig, CircuitData, CommonCircuitData, VerifierCircuitTarget,
};
use plonky2::plonk::config::{AlgebraicHasher, GenericConfig};
use plonky2::plonk::proof::{ProofWithPublicInputs, ProofWithPublicInputsTarget};
use plonky2::recursion::dummy_circuit::cyclic_base_proof;

// Generates `CommonCircuitData` usable for recursion.
pub fn common_data_for_recursion<
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    const D: usize,
>() -> CommonCircuitData<F, D>
where
    C::Hasher: AlgebraicHasher<F>, {
    let config = CircuitConfig::standard_recursion_config();
    let builder = CircuitBuilder::<F, D>::new(config);
    let data = builder.build::<C>();

    let config = CircuitConfig::standard_recursion_config();
    let mut builder = CircuitBuilder::<F, D>::new(config);
    let proof = builder.add_virtual_proof_with_pis(&data.common);
    let verifier_data = builder.add_virtual_verifier_data(data.common.config.fri_config.cap_height);
    builder.verify_proof::<C>(&proof, &verifier_data, &data.common);
    let data = builder.build::<C>();

    let config = CircuitConfig::standard_recursion_config();
    let mut builder = CircuitBuilder::<F, D>::new(config);
    let proof = builder.add_virtual_proof_with_pis(&data.common);
    let verifier_data = builder.add_virtual_verifier_data(data.common.config.fri_config.cap_height);
    builder.verify_proof::<C>(&proof, &verifier_data, &data.common);
    while builder.num_gates() < 1 << 12 {
        builder.add_gate(NoopGate, vec![]);
    }
    builder.build::<C>().common
}

pub struct MakeTreeLeafCircuit<F, C, const D: usize>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>, {
    pub common_data: CommonCircuitData<F, D>,
    pub circuit: CircuitData<F, C, D>,
    pub targets: MakeTreeTargets<D>,
}

pub struct MakeTreeBranchCircuit<F, C, const D: usize>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>, {
    pub common_data: CommonCircuitData<F, D>,
    pub circuit: CircuitData<F, C, D>,
    pub targets: MakeTreeTargets<D>,
}

pub struct MakeTreeTargets<const D: usize> {
    pub hash: HashOutTarget,
    pub leaf_value: HashOutTarget,
    pub verifier_data_target: VerifierCircuitTarget,
    pub left_proof: ProofWithPublicInputsTarget<D>,
    // pub right_proof: ProofWithPublicInputsTarget<D>,
}

impl<F, C, const D: usize> MakeTreeLeafCircuit<F, C, D>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F> + 'static,
    C::Hasher: AlgebraicHasher<F>,
{
    #[must_use]
    pub fn new(circuit_config: &CircuitConfig) -> Result<Self> {
        let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());
        let mut common_data = common_data_for_recursion::<F, C, D>();

        let hash = builder.add_virtual_hash();
        let leaf_value = builder.add_virtual_hash();
        builder.register_public_inputs(&hash.elements);
        builder.register_public_inputs(&leaf_value.elements);

        // let is_leaf =
        // hash.elements.into_iter_fixed().zip(leaf_value.elements).map(|(hash,
        // leaf_value)| builder.is_equal(hash, leaf_value)); let is_leaf: [_;
        // NUM_HASH_OUT_ELTS] = is_leaf.collect(); let is_leaf = [
        //     builder.and(is_leaf[0], is_leaf[1]),
        //     builder.and(is_leaf[2], is_leaf[3]),
        // ];
        // let is_leaf = builder.and(is_leaf[0], is_leaf[1]);

        let verifier_data_target = builder.add_verifier_data_public_inputs();
        common_data.num_public_inputs = builder.num_public_inputs();
        let is_leaf = builder.add_virtual_bool_target_safe();

        let left_proof = builder.add_virtual_proof_with_pis(&common_data);
        // let right_proof = builder.add_virtual_proof_with_pis(&common_data);

        builder.conditionally_verify_cyclic_proof_or_dummy::<C>(
            is_leaf,
            &left_proof,
            &common_data,
        )?;
        // builder.conditionally_verify_cyclic_proof_or_dummy::<C>(is_leaf,
        // &right_proof, &common_data)?; builder.verify_proof::<C>(&left_proof,
        // &verifier_data_target, &common_data); builder.verify_proof::<C>(&
        // right_proof, &verifier_data_target, &common_data);

        let targets = MakeTreeTargets {
            hash,
            leaf_value,
            verifier_data_target,
            left_proof,
        };
        let circuit = builder.build();

        Ok(Self {
            common_data,
            circuit,
            targets,
        })
    }

    pub fn prove(&self, hash: HashOut<F>) -> Result<ProofWithPublicInputs<F, C, D>> {
        let mut inputs = PartialWitness::new();
        inputs.set_hash_target(self.targets.hash, hash);
        let initial_hash = [F::ZERO; NUM_HASH_OUT_ELTS];
        let initial_hash_pis = initial_hash.into_iter().enumerate().collect();
        inputs.set_proof_with_pis_target::<C, D>(
            &self.targets.left_proof,
            &cyclic_base_proof(
                &self.common_data,
                &self.circuit.verifier_only,
                initial_hash_pis,
            ),
        );
        // let initial_hash = [F::ZERO; NUM_HASH_OUT_ELTS];
        // let initial_hash_pis = initial_hash.into_iter().enumerate().collect();
        // inputs.set_proof_with_pis_target::<C, D>(
        //     &self.targets.right_proof,
        //     &cyclic_base_proof(
        //         &self.common_data,
        //         &self.circuit.verifier_only,
        //         initial_hash_pis,
        //     ),
        // );
        inputs.set_verifier_data_target(
            &self.targets.verifier_data_target,
            &self.circuit.verifier_only,
        );
        self.circuit.prove(inputs)
    }
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use plonky2::field::types::Field;
    use plonky2::hash::hash_types::{HashOut, NUM_HASH_OUT_ELTS};
    use plonky2::plonk::circuit_data::CircuitConfig;

    use super::*;
    use crate::test_utils::{hash_branch, hash_str, C, D, F};

    #[test]
    fn verify_leaf() -> Result<()> {
        let circuit_config = CircuitConfig::standard_recursion_config();
        let circuit = MakeTreeLeafCircuit::<F, C, D>::new(&circuit_config)?;

        let zero_hash = HashOut::from([F::ZERO; NUM_HASH_OUT_ELTS]);
        let non_zero_hash = hash_str("Non-Zero Hash");

        let proof = circuit.prove(zero_hash)?;
        circuit.circuit.verify(proof)?;

        let proof = circuit.prove(non_zero_hash)?;
        circuit.circuit.verify(proof)?;

        Ok(())
    }
}

use anyhow::Result;
use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::{HashOut, HashOutTarget, RichField};
use plonky2::hash::poseidon2::Poseidon2Hash;
use plonky2::iop::witness::{PartialWitness, WitnessWrite};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::{CircuitConfig, CircuitData};
use plonky2::plonk::config::{AlgebraicHasher, GenericConfig};
use plonky2::plonk::proof::{ProofWithPublicInputs, ProofWithPublicInputsTarget};

use super::{at_least_one_true, hashes_equal, summarized, unpruned, verify_address};
pub struct LeafCircuit<F, C, const D: usize>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>, {
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

        let summarized_inputs = summarized::SubCircuitInputs::default(&mut builder);
        let old_inputs = unpruned::SubCircuitInputs::default(&mut builder);
        let new_inputs = unpruned::SubCircuitInputs::default(&mut builder);
        let address_inputs = verify_address::LeafInputs {
            node_address: builder.add_virtual_target(),
            node_present: summarized_inputs.summary_hash_present,
        };
        builder.register_public_input(address_inputs.node_address);

        let summarized_targets = summarized_inputs.build_leaf(&mut builder);
        let old_targets = old_inputs.build_leaf(&mut builder);
        let new_targets = new_inputs.build_leaf(&mut builder);
        let address_targets = address_inputs.build(&mut builder);

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

        let summarized = summarized_targets.build(&circuit.prover_only.public_inputs);
        let old = old_targets.build(&circuit.prover_only.public_inputs);
        let new = new_targets.build(&circuit.prover_only.public_inputs);
        let address = address_targets.build(&circuit.prover_only.public_inputs);

        Self {
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
        summary_hash: HashOut<F>,
        address: Option<u64>,
    ) -> Result<ProofWithPublicInputs<F, C, D>> {
        let mut inputs = PartialWitness::new();
        self.summarized.set_inputs(&mut inputs, summary_hash);
        self.old.set_inputs(&mut inputs, old_hash);
        self.new.set_inputs(&mut inputs, new_hash);
        self.address.set_inputs(&mut inputs, address);
        self.circuit.prove(inputs)
    }
}

pub struct BranchCircuit<F, C, const D: usize>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>, {
    pub summarized: summarized::BranchSubCircuit,
    pub old: unpruned::BranchSubCircuit,
    pub new: unpruned::BranchSubCircuit,
    pub address: verify_address::BranchSubCircuit,
    pub circuit: CircuitData<F, C, D>,
    pub targets: BranchTargets<D>,
}

pub struct BranchTargets<const D: usize> {
    pub left_proof: ProofWithPublicInputsTarget<D>,
    pub right_proof: ProofWithPublicInputsTarget<D>,
}

impl<F, C, const D: usize> BranchCircuit<F, C, D>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    <C as GenericConfig<D>>::Hasher: AlgebraicHasher<F>,
{
    #[must_use]
    pub fn from_leaf(circuit_config: &CircuitConfig, leaf: &LeafCircuit<F, C, D>) -> Self {
        let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());
        let common = &leaf.circuit.common;
        let verifier = builder.constant_verifier_data(&leaf.circuit.verifier_only);
        let left_proof = builder.add_virtual_proof_with_pis(common);
        let right_proof = builder.add_virtual_proof_with_pis(common);
        let summarized_inputs = summarized::SubCircuitInputs::default(&mut builder);
        let old_inputs = unpruned::SubCircuitInputs::default(&mut builder);
        let new_inputs = unpruned::SubCircuitInputs::default(&mut builder);
        let address_inputs = verify_address::BranchInputs {
            node_address: builder.add_virtual_target(),
            node_present: summarized_inputs.summary_hash_present,
        };
        builder.register_public_input(address_inputs.node_address);

        builder.verify_proof::<C>(&left_proof, &verifier, common);
        builder.verify_proof::<C>(&right_proof, &verifier, common);
        let summarized_targets =
            summarized_inputs.from_leaf(&mut builder, &leaf.summarized, &left_proof, &right_proof);
        let old_targets = old_inputs.from_leaf(&mut builder, &leaf.old, &left_proof, &right_proof);
        let new_targets = new_inputs.from_leaf(&mut builder, &leaf.new, &left_proof, &right_proof);
        let address_targets =
            address_inputs.from_leaf(&mut builder, &leaf.address, &left_proof, &right_proof);
        let targets = BranchTargets {
            left_proof,
            right_proof,
        };

        let circuit = builder.build();
        let summarized = summarized_targets.from_leaf(&circuit.prover_only.public_inputs);
        let old = old_targets.from_leaf(&circuit.prover_only.public_inputs);
        let new = new_targets.from_leaf(&circuit.prover_only.public_inputs);
        let address = address_targets.from_leaf(&circuit.prover_only.public_inputs);

        Self {
            summarized,
            old,
            new,
            address,
            circuit,
            targets,
        }
    }

    #[must_use]
    pub fn from_branch(circuit_config: &CircuitConfig, branch: &BranchCircuit<F, C, D>) -> Self {
        let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());
        let common = &branch.circuit.common;
        let verifier = builder.constant_verifier_data(&branch.circuit.verifier_only);
        let left_proof = builder.add_virtual_proof_with_pis(common);
        let right_proof = builder.add_virtual_proof_with_pis(common);
        let summarized_inputs = summarized::SubCircuitInputs::default(&mut builder);
        let old_inputs = unpruned::SubCircuitInputs::default(&mut builder);
        let new_inputs = unpruned::SubCircuitInputs::default(&mut builder);
        let address_inputs = verify_address::BranchInputs {
            node_address: builder.add_virtual_target(),
            node_present: summarized_inputs.summary_hash_present,
        };
        builder.register_public_input(address_inputs.node_address);

        builder.verify_proof::<C>(&left_proof, &verifier, common);
        builder.verify_proof::<C>(&right_proof, &verifier, common);
        let summarized_targets = summarized_inputs.from_branch(
            &mut builder,
            &branch.summarized,
            &left_proof,
            &right_proof,
        );
        let old_targets =
            old_inputs.from_branch(&mut builder, &branch.old, &left_proof, &right_proof);
        let new_targets =
            new_inputs.from_branch(&mut builder, &branch.new, &left_proof, &right_proof);
        let address_targets =
            address_inputs.from_branch(&mut builder, &branch.address, &left_proof, &right_proof);
        let targets = BranchTargets {
            left_proof,
            right_proof,
        };

        let circuit = builder.build();
        let summarized =
            summarized_targets.from_branch(&branch.summarized, &circuit.prover_only.public_inputs);
        let old = old_targets.from_branch(&branch.old, &circuit.prover_only.public_inputs);
        let new = new_targets.from_branch(&branch.new, &circuit.prover_only.public_inputs);
        let address = address_targets.from_leaf(&circuit.prover_only.public_inputs);

        Self {
            summarized,
            old,
            new,
            address,
            circuit,
            targets,
        }
    }

    pub fn prove(
        &self,
        left_proof: &ProofWithPublicInputs<F, C, D>,
        right_proof: &ProofWithPublicInputs<F, C, D>,
        old_hash: HashOut<F>,
        new_hash: HashOut<F>,
        summary_hash: HashOut<F>,
        address: impl Into<AddressPresent>,
    ) -> Result<ProofWithPublicInputs<F, C, D>>
    where
        <C as GenericConfig<D>>::Hasher: AlgebraicHasher<F>, {
        let mut inputs = PartialWitness::new();
        inputs.set_proof_with_pis_target(&self.targets.left_proof, left_proof);
        inputs.set_proof_with_pis_target(&self.targets.right_proof, right_proof);
        self.summarized.set_inputs(&mut inputs, summary_hash);
        self.old.set_inputs(&mut inputs, old_hash);
        self.new.set_inputs(&mut inputs, new_hash);
        match address.into() {
            AddressPresent::Present(a) => self.address.set_inputs(&mut inputs, Some(a)),
            AddressPresent::Absent => self.address.set_inputs(&mut inputs, None),
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
mod test {
    use anyhow::Result;
    use plonky2::field::types::Field;
    use plonky2::plonk::circuit_data::CircuitConfig;
    use plonky2::plonk::config::Hasher;

    use super::*;
    use crate::test_utils::{hash_branch, hash_str, C, D, F};

    fn hash_write<F: RichField>(address: u64, left: &HashOut<F>, right: &HashOut<F>) -> HashOut<F> {
        let address = F::from_canonical_u64(address);
        let [l0, l1, l2, l3] = left.elements;
        let [r0, r1, r2, r3] = right.elements;
        Poseidon2Hash::hash_no_pad(&[address, l0, l1, l2, l3, r0, r1, r2, r3])
    }

    #[test]
    fn verify_leaf() -> Result<()> {
        let circuit_config = CircuitConfig::standard_recursion_config();
        let circuit = LeafCircuit::<F, C, D>::new(&circuit_config);

        let zero_hash = HashOut::from([F::ZERO; 4]);
        let non_zero_hash_1 = hash_str("Non-Zero Hash 1");
        let non_zero_hash_2 = hash_str("Non-Zero Hash 2");
        let slot_42_r0w1 = hash_write(42, &zero_hash, &non_zero_hash_1);
        let slot_42_r1w2 = hash_write(42, &non_zero_hash_1, &non_zero_hash_2);
        let slot_42_r2w0 = hash_write(42, &non_zero_hash_2, &zero_hash);

        // Create
        let proof = circuit.prove(zero_hash, non_zero_hash_1, slot_42_r0w1, Some(42))?;
        circuit.circuit.verify(proof)?;

        // Update
        let proof = circuit.prove(non_zero_hash_1, non_zero_hash_2, slot_42_r1w2, Some(42))?;
        circuit.circuit.verify(proof)?;

        // Non-Update
        let proof = circuit.prove(non_zero_hash_2, non_zero_hash_2, zero_hash, None)?;
        circuit.circuit.verify(proof)?;

        // Destroy
        let proof = circuit.prove(non_zero_hash_2, zero_hash, slot_42_r2w0, Some(42))?;
        circuit.circuit.verify(proof)?;

        // Non-Update
        let proof = circuit.prove(zero_hash, zero_hash, zero_hash, None)?;
        circuit.circuit.verify(proof)?;

        Ok(())
    }

    #[test]
    #[should_panic(expected = "Tried to invert zero")]
    fn bad_leaf_create() {
        let circuit_config = CircuitConfig::standard_recursion_config();
        let circuit = LeafCircuit::<F, C, D>::new(&circuit_config);

        let zero_hash = HashOut::from([F::ZERO; 4]);
        let non_zero_hash_1 = hash_str("Non-Zero Hash 1");

        let proof = circuit
            .prove(zero_hash, non_zero_hash_1, zero_hash, None)
            .unwrap();
        circuit.circuit.verify(proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_leaf_update() {
        let circuit_config = CircuitConfig::standard_recursion_config();
        let circuit = LeafCircuit::<F, C, D>::new(&circuit_config);

        let zero_hash = HashOut::from([F::ZERO; 4]);
        let non_zero_hash_1 = hash_str("Non-Zero Hash 1");
        let non_zero_hash_2 = hash_str("Non-Zero Hash 2");
        let hash_0_to_1 = hash_branch(&zero_hash, &non_zero_hash_1);

        let proof = circuit
            .prove(non_zero_hash_1, non_zero_hash_2, hash_0_to_1, Some(42))
            .unwrap();
        circuit.circuit.verify(proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_leaf_non_update() {
        let circuit_config = CircuitConfig::standard_recursion_config();
        let circuit = LeafCircuit::<F, C, D>::new(&circuit_config);

        let non_zero_hash_2 = hash_str("Non-Zero Hash 2");

        let proof = circuit
            .prove(non_zero_hash_2, non_zero_hash_2, non_zero_hash_2, Some(42))
            .unwrap();
        circuit.circuit.verify(proof).unwrap();
    }

    #[test]
    fn verify_branch() -> Result<()> {
        let circuit_config = CircuitConfig::standard_recursion_config();
        let leaf_circuit = LeafCircuit::<F, C, D>::new(&circuit_config);
        let branch_circuit_h0 = BranchCircuit::from_leaf(&circuit_config, &leaf_circuit);
        let branch_circuit_h1 = BranchCircuit::from_branch(&circuit_config, &branch_circuit_h0);

        let zero_hash = HashOut::from([F::ZERO; 4]);
        let non_zero_hash_1 = hash_str("Non-Zero Hash 1");
        let hash_0_and_0 = hash_branch(&zero_hash, &zero_hash);
        let hash_0_and_1 = hash_branch(&zero_hash, &non_zero_hash_1);

        let hash_1_and_0 = hash_branch(&non_zero_hash_1, &zero_hash);
        let hash_1_and_1 = hash_branch(&non_zero_hash_1, &non_zero_hash_1);
        let hash_00_and_00 = hash_branch(&hash_0_and_0, &hash_0_and_0);
        let hash_01_and_10 = hash_branch(&hash_0_and_1, &hash_1_and_0);

        let slot_2_r0w1 = hash_write(2, &zero_hash, &non_zero_hash_1);
        let slot_3_r0w1 = hash_write(3, &zero_hash, &non_zero_hash_1);
        let slot_4_r0w1 = hash_write(4, &zero_hash, &non_zero_hash_1);

        let slot_2_and_3 = hash_branch(&slot_2_r0w1, &slot_3_r0w1);
        let slot_3_and_4 = hash_branch(&slot_3_r0w1, &slot_4_r0w1);

        // Leaf proofs
        let zero_proof = leaf_circuit.prove(zero_hash, zero_hash, zero_hash, None)?;
        leaf_circuit.circuit.verify(zero_proof.clone())?;

        let proof_0_to_1_id_2 =
            leaf_circuit.prove(zero_hash, non_zero_hash_1, slot_2_r0w1, Some(2))?;
        leaf_circuit.circuit.verify(proof_0_to_1_id_2.clone())?;

        let proof_0_to_1_id_3 =
            leaf_circuit.prove(zero_hash, non_zero_hash_1, slot_3_r0w1, Some(3))?;
        leaf_circuit.circuit.verify(proof_0_to_1_id_3.clone())?;

        let proof_0_to_1_id_4 =
            leaf_circuit.prove(zero_hash, non_zero_hash_1, slot_4_r0w1, Some(4))?;
        leaf_circuit.circuit.verify(proof_0_to_1_id_4.clone())?;

        // Branch proofs
        let branch_00_and_00_proof = branch_circuit_h0.prove(
            &zero_proof,
            &zero_proof,
            hash_0_and_0,
            hash_0_and_0,
            zero_hash,
            (),
        )?;
        branch_circuit_h0.circuit.verify(branch_00_and_00_proof)?;

        let branch_00_and_01_proof = branch_circuit_h0.prove(
            &zero_proof,
            &proof_0_to_1_id_3,
            hash_0_and_0,
            hash_0_and_1,
            slot_3_r0w1,
            (),
        )?;
        branch_circuit_h0
            .circuit
            .verify(branch_00_and_01_proof.clone())?;

        let branch_01_and_00_proof = branch_circuit_h0.prove(
            &proof_0_to_1_id_4,
            &zero_proof,
            hash_0_and_0,
            hash_1_and_0,
            slot_4_r0w1,
            (),
        )?;
        branch_circuit_h0
            .circuit
            .verify(branch_01_and_00_proof.clone())?;

        let branch_01_and_01_proof = branch_circuit_h0.prove(
            &proof_0_to_1_id_2,
            &proof_0_to_1_id_3,
            hash_0_and_0,
            hash_1_and_1,
            slot_2_and_3,
            (),
        )?;
        branch_circuit_h0.circuit.verify(branch_01_and_01_proof)?;

        // Double branch proof
        let proof = branch_circuit_h1.prove(
            &branch_00_and_01_proof,
            &branch_01_and_00_proof,
            hash_00_and_00,
            hash_01_and_10,
            slot_3_and_4,
            (),
        )?;
        branch_circuit_h1.circuit.verify(proof)?;

        Ok(())
    }
}

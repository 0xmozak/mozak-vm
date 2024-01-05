use anyhow::Result;
use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::{HashOut, HashOutTarget, RichField};
use plonky2::hash::poseidon2::Poseidon2Hash;
use plonky2::iop::generator::{GeneratedValues, SimpleGenerator};
use plonky2::iop::target::{BoolTarget, Target};
use plonky2::iop::witness::{PartialWitness, PartitionWitness, Witness, WitnessWrite};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::{CircuitConfig, CircuitData, CommonCircuitData};
use plonky2::plonk::config::{AlgebraicHasher, GenericConfig};
use plonky2::plonk::proof::{ProofWithPublicInputs, ProofWithPublicInputsTarget};
use plonky2::util::serialization::{Buffer, IoResult, Read, Write};

pub mod summarized;
pub mod unpruned;

/// A generator for testing if a value equals zero
#[derive(Debug, Default)]
struct NonzeroTestGenerator {
    to_test: Target,
    result: BoolTarget,
}

impl<F: RichField + Extendable<D>, const D: usize> SimpleGenerator<F, D> for NonzeroTestGenerator {
    fn id(&self) -> String { "NonzeroTestGenerator".to_string() }

    fn dependencies(&self) -> Vec<Target> { vec![self.to_test] }

    fn run_once(&self, witness: &PartitionWitness<F>, out_buffer: &mut GeneratedValues<F>) {
        let to_test_value = witness.get_target(self.to_test);
        out_buffer.set_bool_target(self.result, to_test_value.is_nonzero());
    }

    fn serialize(&self, dst: &mut Vec<u8>, _common_data: &CommonCircuitData<F, D>) -> IoResult<()> {
        dst.write_target(self.to_test)?;
        dst.write_target_bool(self.result)
    }

    fn deserialize(src: &mut Buffer, _common_data: &CommonCircuitData<F, D>) -> IoResult<Self> {
        let to_test = src.read_target()?;
        let result = src.read_target_bool()?;
        Ok(Self { to_test, result })
    }
}

fn is_nonzero<F, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    to_test: Target,
) -> BoolTarget
where
    F: RichField + Extendable<D>, {
    // `result = to_test != 0`, meaning it's 0 for `to_test == 0` or 1 for all other
    // to_test we'll represent this as `result = 0 | 1`
    // note that this can be falsely proved so we have to put some constraints below
    // to ensure it
    let result = builder.add_virtual_bool_target_safe();
    builder.add_simple_generator(NonzeroTestGenerator { to_test, result });

    // Enforce the result through arithmetic
    let neg = builder.not(result); // neg = 1 | 0
    let denom = builder.add(to_test, neg.target); // denom = 1 | to_test
    let div = builder.div(to_test, denom); // div = 0 | 1

    builder.connect(result.target, div);

    result
}

pub trait SubCircuit<PublicIndices> {
    fn pis(&self) -> usize;
    fn get_indices(&self) -> PublicIndices;
}

pub struct CompleteLeafCircuit<F, C, const D: usize>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>, {
    pub summarized: summarized::LeafSubCircuit,
    pub old: unpruned::LeafSubCircuit,
    pub new: unpruned::LeafSubCircuit,
    pub circuit: CircuitData<F, C, D>,
}

impl<F, C, const D: usize> CompleteLeafCircuit<F, C, D>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
{
    #[must_use]
    pub fn new(circuit_config: &CircuitConfig) -> Self {
        let builder = CircuitBuilder::<F, D>::new(circuit_config.clone());
        let (circuit, (summarized, (old, (new, ())))) =
            summarized::LeafSubCircuit::new(builder, |summarized_targets, builder| {
                unpruned::LeafSubCircuit::new(builder, |old_targets, builder| {
                    unpruned::LeafSubCircuit::new(builder, |new_targets, mut builder| {
                        // Summarize both old and new by hashing them together
                        let old_new_parent = builder.hash_n_to_hash_no_pad::<Poseidon2Hash>(
                            old_targets
                                .unpruned_hash
                                .elements
                                .into_iter()
                                .chain(new_targets.unpruned_hash.elements)
                                .collect(),
                        );

                        // zero it out based on if this node is being summarized
                        let old_new_parent = old_new_parent.elements.map(|e| {
                            builder.mul(e, summarized_targets.summary_hash_present.target)
                        });

                        // This should be the summary hash
                        builder.connect_hashes(
                            HashOutTarget::from(old_new_parent),
                            summarized_targets.summary_hash,
                        );

                        // Ensure the presence is based on if there's any change
                        let unchanged = [0, 1, 2, 3].map(|i| {
                            builder.is_equal(
                                old_targets.unpruned_hash.elements[i],
                                new_targets.unpruned_hash.elements[i],
                            )
                        });
                        let unchanged = [
                            builder.and(unchanged[0], unchanged[1]),
                            builder.and(unchanged[2], unchanged[3]),
                        ];
                        let unchanged = builder.and(unchanged[0], unchanged[1]);
                        let changed = builder.not(unchanged);
                        builder.connect(
                            changed.target,
                            summarized_targets.summary_hash_present.target,
                        );

                        (builder.build(), ())
                    })
                })
            });

        Self {
            summarized,
            old,
            new,
            circuit,
        }
    }

    pub fn prove(
        &self,
        old_hash: HashOut<F>,
        new_hash: HashOut<F>,
        summary_hash: HashOut<F>,
    ) -> Result<ProofWithPublicInputs<F, C, D>> {
        let mut inputs = PartialWitness::new();
        self.summarized.set_inputs(&mut inputs, summary_hash);
        self.old.set_inputs(&mut inputs, old_hash);
        self.new.set_inputs(&mut inputs, new_hash);
        self.circuit.prove(inputs)
    }
}

pub struct CompleteBranchCircuit<'a, F, C, const D: usize>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>, {
    pub summarized: summarized::BranchSubCircuit<'a, D>,
    pub old: unpruned::BranchSubCircuit<'a, D>,
    pub new: unpruned::BranchSubCircuit<'a, D>,
    pub circuit: CircuitData<F, C, D>,
    pub targets: CompleteBranchTargets<D>,
}

pub struct CompleteBranchTargets<const D: usize> {
    pub left_proof: ProofWithPublicInputsTarget<D>,
    pub right_proof: ProofWithPublicInputsTarget<D>,
}

impl<'a, F, C, const D: usize> CompleteBranchCircuit<'a, F, C, D>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
{
    #[must_use]
    pub fn from_leaf(
        circuit_config: &CircuitConfig,
        leaf: &'a CompleteLeafCircuit<F, C, D>,
    ) -> Self {
        let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());
        let left_proof = builder.add_virtual_proof_with_pis(&leaf.circuit.common);
        let right_proof = builder.add_virtual_proof_with_pis(&leaf.circuit.common);

        let (circuit, (summarized, (old, (new, ())))) = summarized::BranchSubCircuit::from_leaf(
            builder,
            &leaf.summarized,
            &left_proof,
            &right_proof,
            |_summarized_targets, builder| {
                unpruned::BranchSubCircuit::from_leaf(
                    builder,
                    &leaf.old,
                    &left_proof,
                    &right_proof,
                    |_old_targets, builder| {
                        unpruned::BranchSubCircuit::from_leaf(
                            builder,
                            &leaf.new,
                            &left_proof,
                            &right_proof,
                            |_new_targets, builder| (builder.build(), ()),
                        )
                    },
                )
            },
        );

        Self {
            summarized,
            old,
            new,
            circuit,
            targets: CompleteBranchTargets {
                left_proof,
                right_proof,
            },
        }
    }

    #[must_use]
    pub fn from_branch(
        circuit_config: &CircuitConfig,
        branch: &'a CompleteBranchCircuit<'a, F, C, D>,
    ) -> Self {
        let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());
        let left_proof = builder.add_virtual_proof_with_pis(&branch.circuit.common);
        let right_proof = builder.add_virtual_proof_with_pis(&branch.circuit.common);

        let (circuit, (summarized, (old, (new, ())))) = summarized::BranchSubCircuit::from_branch(
            builder,
            &branch.summarized,
            &left_proof,
            &right_proof,
            |_summarized_targets, builder| {
                unpruned::BranchSubCircuit::from_branch(
                    builder,
                    &branch.old,
                    &left_proof,
                    &right_proof,
                    |_old_targets, builder| {
                        unpruned::BranchSubCircuit::from_branch(
                            builder,
                            &branch.new,
                            &left_proof,
                            &right_proof,
                            |_new_targets, builder| (builder.build(), ()),
                        )
                    },
                )
            },
        );

        Self {
            summarized,
            old,
            new,
            circuit,
            targets: CompleteBranchTargets {
                left_proof,
                right_proof,
            },
        }
    }

    pub fn prove(
        &self,
        left_proof: &ProofWithPublicInputs<F, C, D>,
        right_proof: &ProofWithPublicInputs<F, C, D>,
        old_hash: HashOut<F>,
        new_hash: HashOut<F>,
        summary_hash: HashOut<F>,
    ) -> Result<ProofWithPublicInputs<F, C, D>>
    where
        <C as GenericConfig<D>>::Hasher: AlgebraicHasher<F>, {
        let mut inputs = PartialWitness::new();
        inputs.set_proof_with_pis_target(&self.targets.left_proof, left_proof);
        inputs.set_proof_with_pis_target(&self.targets.right_proof, right_proof);
        self.summarized.set_inputs(&mut inputs, summary_hash);
        self.old.set_inputs(&mut inputs, old_hash);
        self.new.set_inputs(&mut inputs, new_hash);
        self.circuit.prove(inputs)
    }
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use plonky2::field::types::Field;
    use plonky2::plonk::circuit_data::CircuitConfig;

    use super::*;
    use crate::test_utils::{hash_branch, hash_str, C, D, F};

    #[test]
    fn verify_leaf() -> Result<()> {
        let circuit_config = CircuitConfig::standard_recursion_config();
        let circuit = CompleteLeafCircuit::<F, C, D>::new(&circuit_config);

        let zero_hash = HashOut::from([F::ZERO; 4]);
        let non_zero_hash_1 = hash_str("Non-Zero Hash 1");
        let non_zero_hash_2 = hash_str("Non-Zero Hash 2");
        let hash_0_to_1 = hash_branch(&zero_hash, &non_zero_hash_1);
        let hash_1_to_2 = hash_branch(&non_zero_hash_1, &non_zero_hash_2);
        let hash_2_to_0 = hash_branch(&non_zero_hash_2, &zero_hash);

        // Create
        let proof = circuit.prove(zero_hash, non_zero_hash_1, hash_0_to_1)?;
        circuit.circuit.verify(proof)?;

        // Update
        let proof = circuit.prove(non_zero_hash_1, non_zero_hash_2, hash_1_to_2)?;
        circuit.circuit.verify(proof)?;

        // Non-Update
        let proof = circuit.prove(non_zero_hash_2, non_zero_hash_2, zero_hash)?;
        circuit.circuit.verify(proof)?;

        // Destroy
        let proof = circuit.prove(non_zero_hash_2, zero_hash, hash_2_to_0)?;
        circuit.circuit.verify(proof)?;

        // Non-Update
        let proof = circuit.prove(zero_hash, zero_hash, zero_hash)?;
        circuit.circuit.verify(proof)?;

        Ok(())
    }

    #[test]
    fn verify_branch() -> Result<()> {
        let circuit_config = CircuitConfig::standard_recursion_config();
        let leaf_circuit = CompleteLeafCircuit::<F, C, D>::new(&circuit_config);
        let branch_circuit_1 = CompleteBranchCircuit::from_leaf(&circuit_config, &leaf_circuit);

        let zero_hash = HashOut::from([F::ZERO; 4]);
        let non_zero_hash_1 = hash_str("Non-Zero Hash 1");
        let hash_0_to_0 = hash_branch(&zero_hash, &zero_hash);
        let hash_0_to_1 = hash_branch(&zero_hash, &non_zero_hash_1);
        let hash_1_to_0 = hash_branch(&non_zero_hash_1, &zero_hash);
        let hash_1_to_1 = hash_branch(&non_zero_hash_1, &non_zero_hash_1);
        let hash_01_to_01 = hash_branch(&hash_0_to_1, &hash_0_to_1);

        // Leaf proofs
        let zero_proof = leaf_circuit.prove(zero_hash, zero_hash, zero_hash)?;
        leaf_circuit.circuit.verify(zero_proof.clone())?;

        let proof_0_to_1 = leaf_circuit.prove(zero_hash, non_zero_hash_1, hash_0_to_1)?;
        leaf_circuit.circuit.verify(proof_0_to_1.clone())?;

        // Branch proofs
        let branch_00_and_00_proof = branch_circuit_1.prove(
            &zero_proof,
            &zero_proof,
            hash_0_to_0,
            hash_0_to_0,
            zero_hash,
        )?;
        branch_circuit_1.circuit.verify(branch_00_and_00_proof)?;

        let branch_00_and_01_proof = branch_circuit_1.prove(
            &zero_proof,
            &proof_0_to_1,
            hash_0_to_0,
            hash_0_to_1,
            hash_0_to_1,
        )?;
        branch_circuit_1.circuit.verify(branch_00_and_01_proof)?;

        let branch_01_and_00_proof = branch_circuit_1.prove(
            &proof_0_to_1,
            &zero_proof,
            hash_0_to_0,
            hash_1_to_0,
            hash_0_to_1,
        )?;
        branch_circuit_1.circuit.verify(branch_01_and_00_proof)?;

        let branch_01_and_01_proof = branch_circuit_1.prove(
            &proof_0_to_1,
            &proof_0_to_1,
            hash_0_to_0,
            hash_1_to_1,
            hash_01_to_01,
        )?;
        branch_circuit_1.circuit.verify(branch_01_and_01_proof)?;

        Ok(())
    }
}

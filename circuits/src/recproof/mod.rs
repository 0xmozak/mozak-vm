use anyhow::Result;
use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::{HashOut, RichField};
use plonky2::iop::witness::PartialWitness;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::{CircuitConfig, CircuitData};
use plonky2::plonk::config::GenericConfig;
use plonky2::plonk::proof::{ProofWithPublicInputs, ProofWithPublicInputsTarget};

pub mod summarized;
pub mod unpruned;

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
            summarized::LeafSubCircuit::new(builder, (), |(), summarized_targets, builder| {
                unpruned::LeafSubCircuit::new(
                    builder,
                    summarized_targets,
                    |summarized_targets, old_targets, builder| {
                        unpruned::LeafSubCircuit::new(
                            builder,
                            (summarized_targets, old_targets),
                            |(summarized_targets, _old_targets), new_targets, mut builder| {
                                builder.connect_hashes(
                                    summarized_targets.summary_hash,
                                    new_targets.unpruned_hash,
                                );
                                (builder.build(), ())
                            },
                        )
                    },
                )
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
    ) -> Result<ProofWithPublicInputs<F, C, D>> {
        let mut inputs = PartialWitness::new();
        self.summarized.set_inputs(&mut inputs, new_hash);
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

#[cfg(test)]
pub mod test {
    use anyhow::Result;
    use plonky2::field::types::Field;
    use plonky2::hash::poseidon2::Poseidon2Hash;
    use plonky2::plonk::circuit_data::CircuitConfig;
    use plonky2::plonk::config::Hasher;

    use super::*;
    use crate::test_utils::{C, D, F};

    pub fn hash_str(v: &str) -> HashOut<F> {
        let v: Vec<_> = v.bytes().map(F::from_canonical_u8).collect();
        Poseidon2Hash::hash_no_pad(&v)
    }

    pub fn hash_branch<F: RichField>(left: &HashOut<F>, right: &HashOut<F>) -> HashOut<F> {
        let [l0, l1, l2, l3] = left.elements;
        let [r0, r1, r2, r3] = right.elements;
        Poseidon2Hash::hash_no_pad(&[l0, l1, l2, l3, r0, r1, r2, r3])
    }

    #[test]
    fn verify_leaf() -> Result<()> {
        let circuit_config = CircuitConfig::standard_recursion_config();
        let circuit = CompleteLeafCircuit::<F, C, D>::new(&circuit_config);

        let zero_hash = HashOut::from([F::ZERO; 4]);
        let non_zero_hash_1 = hash_str("Non-Zero Hash 1");
        let non_zero_hash_2 = hash_str("Non-Zero Hash 2");

        // Create
        let proof = circuit.prove(zero_hash, non_zero_hash_1)?;
        circuit.circuit.verify(proof)?;

        // Update
        let proof = circuit.prove(non_zero_hash_1, non_zero_hash_2)?;
        circuit.circuit.verify(proof)?;

        // Destroy
        let proof = circuit.prove(non_zero_hash_2, zero_hash)?;
        circuit.circuit.verify(proof)?;

        Ok(())
    }
}

use anyhow::Result;
use iter_fixed::IntoIteratorFixed;
use itertools::Itertools;
use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::{
    HashOut, HashOutTarget, MerkleCapTarget, RichField, NUM_HASH_OUT_ELTS,
};
use plonky2::hash::poseidon2::Poseidon2Hash;
use plonky2::iop::target::{BoolTarget, Target};
use plonky2::iop::witness::{PartialWitness, WitnessWrite};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::{CircuitConfig, CircuitData, VerifierCircuitTarget};
use plonky2::plonk::config::{AlgebraicHasher, GenericConfig};
use plonky2::plonk::proof::{ProofWithPublicInputs, ProofWithPublicInputsTarget};

pub mod make_tree;
pub mod merge;
pub mod summarized;
pub mod unbounded;
pub mod unpruned;

/// Computes `if b { h0 } else { h1 }`.
pub(crate) fn select_hash<F, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    b: BoolTarget,
    h0: HashOutTarget,
    h1: HashOutTarget,
) -> HashOutTarget
where
    F: RichField + Extendable<D>, {
    HashOutTarget {
        elements: core::array::from_fn(|i| builder.select(b, h0.elements[i], h1.elements[i])),
    }
}

/// Computes `if b { cap0 } else { cap1 }`.
pub(crate) fn select_cap<F, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    b: BoolTarget,
    cap0: &MerkleCapTarget,
    cap1: &MerkleCapTarget,
) -> MerkleCapTarget
where
    F: RichField + Extendable<D>, {
    assert_eq!(cap0.0.len(), cap1.0.len());
    MerkleCapTarget(
        cap0.0
            .iter()
            .zip_eq(&cap1.0)
            .map(|(h0, h1)| select_hash(builder, b, *h0, *h1))
            .collect(),
    )
}

/// Computes `if b { v0 } else { v1 }`.
pub(crate) fn select_verifier<F, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    b: BoolTarget,
    v0: &VerifierCircuitTarget,
    v1: &VerifierCircuitTarget,
) -> VerifierCircuitTarget
where
    F: RichField + Extendable<D>, {
    VerifierCircuitTarget {
        constants_sigmas_cap: select_cap(
            builder,
            b,
            &v0.constants_sigmas_cap,
            &v1.constants_sigmas_cap,
        ),
        circuit_digest: select_hash(builder, b, v0.circuit_digest, v1.circuit_digest),
    }
}

/// Reduce a hash-sized group of booleans by `&&`ing them together
fn and_helper<F, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    bools: [BoolTarget; 4],
) -> BoolTarget
where
    F: RichField + Extendable<D>, {
    let bools = [
        builder.and(bools[0], bools[1]),
        builder.and(bools[2], bools[3]),
    ];
    builder.and(bools[0], bools[1])
}

/// Reduce a hash-sized group of booleans by `||`ing them together
fn or_helper<F, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    bools: [BoolTarget; 4],
) -> BoolTarget
where
    F: RichField + Extendable<D>, {
    let bools = [
        builder.or(bools[0], bools[1]),
        builder.or(bools[2], bools[3]),
    ];
    builder.or(bools[0], bools[1])
}

/// Computes `h0 == h1`.
fn hashes_equal<F, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    h0: HashOutTarget,
    h1: HashOutTarget,
) -> BoolTarget
where
    F: RichField + Extendable<D>, {
    let eq = h0
        .elements
        .into_iter_fixed()
        .zip(h1.elements)
        .map(|(h0, h1)| builder.is_equal(h0, h1))
        .collect();
    and_helper(builder, eq)
}

/// Hash left and right together if both are present, otherwise forward one
fn hash_or_forward<F, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    left_present: BoolTarget,
    left: [Target; NUM_HASH_OUT_ELTS],
    right_present: BoolTarget,
    right: [Target; NUM_HASH_OUT_ELTS],
) -> HashOutTarget
where
    F: RichField + Extendable<D>, {
    let both_present = builder.and(left_present, right_present);

    // Construct the hash of [left, right]
    let hash_both =
        builder.hash_n_to_hash_no_pad::<Poseidon2Hash>(left.into_iter().chain(right).collect());

    // Construct the forwarding "hash".
    let hash_absent = left
        .into_iter_fixed()
        .zip(right)
        // Since absent sides will be zero, we can just sum.
        .map(|(l, r)| builder.add(l, r))
        .collect();
    let hash_absent = HashOutTarget {
        elements: hash_absent,
    };

    // Select the hash based on presence
    select_hash(builder, both_present, hash_both, hash_absent)
}

/// `hash_or_forward` but using non-zero to determine presence
fn hash_or_forward_zero<F, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    left: [Target; NUM_HASH_OUT_ELTS],
    right: [Target; NUM_HASH_OUT_ELTS],
) -> HashOutTarget
where
    F: RichField + Extendable<D>, {
    let left_non_zero = left
        .into_iter_fixed()
        .map(|l_hash| builder.is_nonzero(l_hash))
        .collect();
    let right_non_zero = right
        .into_iter_fixed()
        .map(|r_hash| builder.is_nonzero(r_hash))
        .collect();

    // If any elements are non-zero, then it's non-zero
    let left_non_zero = or_helper(builder, left_non_zero);
    let right_non_zero = or_helper(builder, right_non_zero);

    // Select the hash based on presence
    hash_or_forward(builder, left_non_zero, left, right_non_zero, right)
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
                        let old_hash = old_targets.unpruned_hash.elements;
                        let new_hash = new_targets.unpruned_hash.elements;

                        // Summarize both old and new by hashing them together
                        let old_new_parent = builder.hash_n_to_hash_no_pad::<Poseidon2Hash>(
                            old_hash.into_iter().chain(new_hash).collect(),
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
                        let unchanged = old_hash
                            .into_iter_fixed()
                            .zip(new_hash)
                            .map(|(old, new)| builder.is_equal(old, new))
                            .collect();
                        let unchanged = and_helper(&mut builder, unchanged);
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

pub struct CompleteBranchCircuit<F, C, const D: usize>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>, {
    pub summarized: summarized::BranchSubCircuit,
    pub old: unpruned::BranchSubCircuit,
    pub new: unpruned::BranchSubCircuit,
    pub circuit: CircuitData<F, C, D>,
    pub targets: CompleteBranchTargets<D>,
}

pub struct CompleteBranchTargets<const D: usize> {
    pub left_proof: ProofWithPublicInputsTarget<D>,
    pub right_proof: ProofWithPublicInputsTarget<D>,
}

impl<F, C, const D: usize> CompleteBranchCircuit<F, C, D>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    <C as GenericConfig<D>>::Hasher: AlgebraicHasher<F>,
{
    #[must_use]
    pub fn from_leaf(circuit_config: &CircuitConfig, leaf: &CompleteLeafCircuit<F, C, D>) -> Self {
        let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());
        let common = &leaf.circuit.common;
        let verifier = builder.constant_verifier_data(&leaf.circuit.verifier_only);
        let left_proof = builder.add_virtual_proof_with_pis(common);
        let right_proof = builder.add_virtual_proof_with_pis(common);
        builder.verify_proof::<C>(&left_proof, &verifier, common);
        builder.verify_proof::<C>(&right_proof, &verifier, common);

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
        branch: &CompleteBranchCircuit<F, C, D>,
    ) -> Self {
        let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());
        let common = &branch.circuit.common;
        let verifier = builder.constant_verifier_data(&branch.circuit.verifier_only);
        let left_proof = builder.add_virtual_proof_with_pis(common);
        let right_proof = builder.add_virtual_proof_with_pis(common);
        builder.verify_proof::<C>(&left_proof, &verifier, common);
        builder.verify_proof::<C>(&right_proof, &verifier, common);

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
    #[should_panic(expected = "was set twice with different values")]
    fn bad_leaf_create() {
        let circuit_config = CircuitConfig::standard_recursion_config();
        let circuit = CompleteLeafCircuit::<F, C, D>::new(&circuit_config);

        let zero_hash = HashOut::from([F::ZERO; 4]);
        let non_zero_hash_1 = hash_str("Non-Zero Hash 1");

        let proof = circuit
            .prove(zero_hash, non_zero_hash_1, zero_hash)
            .unwrap();
        circuit.circuit.verify(proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_leaf_update() {
        let circuit_config = CircuitConfig::standard_recursion_config();
        let circuit = CompleteLeafCircuit::<F, C, D>::new(&circuit_config);

        let zero_hash = HashOut::from([F::ZERO; 4]);
        let non_zero_hash_1 = hash_str("Non-Zero Hash 1");
        let non_zero_hash_2 = hash_str("Non-Zero Hash 2");
        let hash_0_to_1 = hash_branch(&zero_hash, &non_zero_hash_1);

        let proof = circuit
            .prove(non_zero_hash_1, non_zero_hash_2, hash_0_to_1)
            .unwrap();
        circuit.circuit.verify(proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_leaf_non_update() {
        let circuit_config = CircuitConfig::standard_recursion_config();
        let circuit = CompleteLeafCircuit::<F, C, D>::new(&circuit_config);

        let non_zero_hash_2 = hash_str("Non-Zero Hash 2");

        let proof = circuit
            .prove(non_zero_hash_2, non_zero_hash_2, non_zero_hash_2)
            .unwrap();
        circuit.circuit.verify(proof).unwrap();
    }

    #[test]
    fn verify_branch() -> Result<()> {
        let circuit_config = CircuitConfig::standard_recursion_config();
        let leaf_circuit = CompleteLeafCircuit::<F, C, D>::new(&circuit_config);
        let branch_circuit_h0 = CompleteBranchCircuit::from_leaf(&circuit_config, &leaf_circuit);
        let branch_circuit_h1 =
            CompleteBranchCircuit::from_branch(&circuit_config, &branch_circuit_h0);

        let zero_hash = HashOut::from([F::ZERO; 4]);
        let non_zero_hash_1 = hash_str("Non-Zero Hash 1");
        let hash_0_and_0 = hash_branch(&zero_hash, &zero_hash);
        let hash_0_and_1 = hash_branch(&zero_hash, &non_zero_hash_1);
        let hash_1_and_0 = hash_branch(&non_zero_hash_1, &zero_hash);
        let hash_1_and_1 = hash_branch(&non_zero_hash_1, &non_zero_hash_1);
        let hash_00_and_00 = hash_branch(&hash_0_and_0, &hash_0_and_0);
        let hash_01_and_01 = hash_branch(&hash_0_and_1, &hash_0_and_1);
        let hash_01_and_10 = hash_branch(&hash_0_and_1, &hash_1_and_0);

        // Leaf proofs
        let zero_proof = leaf_circuit.prove(zero_hash, zero_hash, zero_hash)?;
        leaf_circuit.circuit.verify(zero_proof.clone())?;

        let proof_0_to_1 = leaf_circuit.prove(zero_hash, non_zero_hash_1, hash_0_and_1)?;
        leaf_circuit.circuit.verify(proof_0_to_1.clone())?;

        // Branch proofs
        let branch_00_and_00_proof = branch_circuit_h0.prove(
            &zero_proof,
            &zero_proof,
            hash_0_and_0,
            hash_0_and_0,
            zero_hash,
        )?;
        branch_circuit_h0.circuit.verify(branch_00_and_00_proof)?;

        let branch_00_and_01_proof = branch_circuit_h0.prove(
            &zero_proof,
            &proof_0_to_1,
            hash_0_and_0,
            hash_0_and_1,
            hash_0_and_1,
        )?;
        branch_circuit_h0
            .circuit
            .verify(branch_00_and_01_proof.clone())?;

        let branch_01_and_00_proof = branch_circuit_h0.prove(
            &proof_0_to_1,
            &zero_proof,
            hash_0_and_0,
            hash_1_and_0,
            hash_0_and_1,
        )?;
        branch_circuit_h0
            .circuit
            .verify(branch_01_and_00_proof.clone())?;

        let branch_01_and_01_proof = branch_circuit_h0.prove(
            &proof_0_to_1,
            &proof_0_to_1,
            hash_0_and_0,
            hash_1_and_1,
            hash_01_and_01,
        )?;
        branch_circuit_h0.circuit.verify(branch_01_and_01_proof)?;

        // Double branch proof
        let proof = branch_circuit_h1.prove(
            &branch_00_and_01_proof,
            &branch_01_and_00_proof,
            hash_00_and_00,
            hash_01_and_10,
            hash_01_and_01,
        )?;
        branch_circuit_h1.circuit.verify(proof)?;

        Ok(())
    }
}

//! Subcircuits for recursively proving the entire contents of a merkle tree.
//!
//! These subcircuits are useful because with just a pair of them, say a old and
//! new, you can prove a transition from the current merkle root (proved by old)
//! to a new merkle root (proved by new).

use itertools::chain;
use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::{HashOut, HashOutTarget, RichField};
use plonky2::hash::poseidon2::Poseidon2Hash;
use plonky2::iop::target::{BoolTarget, Target};
use plonky2::iop::witness::{PartialWitness, WitnessWrite};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::proof::ProofWithPublicInputsTarget;

use crate::indices::HashTargetIndex;
use crate::{byte_wise_hash, select_hash};

pub trait Extended {
    type BranchTargets;
}
pub type ExtendedBranchTargets<E> = <E as Extended>::BranchTargets;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct OnlyFull;
impl Extended for OnlyFull {
    type BranchTargets = ();
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct PartialAllowed;
impl Extended for PartialAllowed {
    type BranchTargets = BranchTargetsExtension;
}

pub struct BranchTargetsExtension {
    /// Whether or not the right target is present
    pub partial: BoolTarget,
}

/// The indices of the public inputs of this subcircuit in any
/// `ProofWithPublicInputs`
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct PublicIndices {
    /// The indices of each of the elements of the unpruned hash
    pub unpruned_hash: HashTargetIndex,
}

pub struct SubCircuitInputs {
    /// The hash of the unpruned state or ZERO if absent
    /// For leafs this is just an arbitrary values
    /// For branches this is the hash of `[left.unpruned_hash,
    /// right.unpruned_hash]`
    pub unpruned_hash: HashOutTarget,
}

pub struct LeafTargets {
    /// The public inputs
    pub inputs: SubCircuitInputs,
}

impl SubCircuitInputs {
    pub fn default<F, const D: usize>(builder: &mut CircuitBuilder<F, D>) -> Self
    where
        F: RichField + Extendable<D>, {
        let unpruned_hash = builder.add_virtual_hash();
        builder.register_public_inputs(&unpruned_hash.elements);
        Self { unpruned_hash }
    }

    #[must_use]
    pub fn build_leaf<F, const D: usize>(self, _builder: &mut CircuitBuilder<F, D>) -> LeafTargets
    where
        F: RichField + Extendable<D>, {
        LeafTargets { inputs: self }
    }
}

/// The leaf subcircuit metadata. This subcircuit does basically nothing, simply
/// expressing that a hash exists
pub struct LeafSubCircuit {
    pub targets: LeafTargets,
    pub indices: PublicIndices,
}

impl LeafTargets {
    #[must_use]
    pub fn build(self, public_inputs: &[Target]) -> LeafSubCircuit {
        let indices = PublicIndices {
            unpruned_hash: HashTargetIndex::new(public_inputs, self.inputs.unpruned_hash),
        };
        LeafSubCircuit {
            targets: self,
            indices,
        }
    }
}

impl LeafSubCircuit {
    /// Get ready to generate a proof
    pub fn set_witness<F: RichField>(
        &self,
        inputs: &mut PartialWitness<F>,
        unpruned_hash: HashOut<F>,
    ) {
        inputs.set_hash_target(self.targets.inputs.unpruned_hash, unpruned_hash);
    }
}

pub struct BranchTargets<E: Extended> {
    /// The public inputs
    pub inputs: SubCircuitInputs,

    /// The left direction
    pub left: SubCircuitInputs,

    /// The right direction
    pub right: SubCircuitInputs,

    /// The extended targets
    pub extension: ExtendedBranchTargets<E>,
}

impl SubCircuitInputs {
    #[must_use]
    fn helper<F: RichField + Extendable<D>, const D: usize, E: Extended>(
        self,
        builder: &mut CircuitBuilder<F, D>,
        indices: &PublicIndices,
        left_proof: &ProofWithPublicInputsTarget<D>,
        right_proof: &ProofWithPublicInputsTarget<D>,
        vm_hashing: bool,
        left_or_hash: impl FnOnce(
            &mut CircuitBuilder<F, D>,
            HashOutTarget,
            HashOutTarget,
        ) -> (HashOutTarget, ExtendedBranchTargets<E>),
    ) -> BranchTargets<E> {
        let hasher = if vm_hashing {
            byte_wise_hash
        } else {
            CircuitBuilder::hash_n_to_hash_no_pad::<Poseidon2Hash>
        };

        let l_values = indices.unpruned_hash.get_any(&left_proof.public_inputs);
        let r_values = indices.unpruned_hash.get_any(&right_proof.public_inputs);

        // Hash the left and right together
        let unpruned_hash_calc = hasher(builder, chain!(l_values, r_values).collect());

        let left = SubCircuitInputs {
            unpruned_hash: HashOutTarget::from(l_values),
        };
        let right = SubCircuitInputs {
            unpruned_hash: HashOutTarget::from(r_values),
        };

        // Apply any extensions
        let (unpruned_hash_calc, extension) =
            left_or_hash(builder, left.unpruned_hash, unpruned_hash_calc);
        builder.connect_hashes(unpruned_hash_calc, self.unpruned_hash);

        BranchTargets {
            inputs: self,
            left,
            right,
            extension,
        }
    }

    #[must_use]
    pub fn build_branch<F: RichField + Extendable<D>, const D: usize>(
        self,
        builder: &mut CircuitBuilder<F, D>,
        indices: &PublicIndices,
        left_proof: &ProofWithPublicInputsTarget<D>,
        right_proof: &ProofWithPublicInputsTarget<D>,
        vm_hashing: bool,
    ) -> BranchTargets<OnlyFull> {
        self.helper(
            builder,
            indices,
            left_proof,
            right_proof,
            vm_hashing,
            |_builder, _left, unpruned_hash_calc| (unpruned_hash_calc, ()),
        )
    }

    #[must_use]
    pub fn build_extended_branch<F: RichField + Extendable<D>, const D: usize>(
        self,
        builder: &mut CircuitBuilder<F, D>,
        indices: &PublicIndices,
        left_proof: &ProofWithPublicInputsTarget<D>,
        right_proof: &ProofWithPublicInputsTarget<D>,
        vm_hashing: bool,
    ) -> BranchTargets<PartialAllowed> {
        let partial = builder.add_virtual_bool_target_safe();
        self.helper(
            builder,
            indices,
            left_proof,
            right_proof,
            vm_hashing,
            |builder, left, unpruned_hash_calc| {
                (
                    select_hash(builder, partial, left, unpruned_hash_calc),
                    BranchTargetsExtension { partial },
                )
            },
        )
    }
}

/// The branch subcircuit metadata. This subcircuit proves knowledge of two
/// private subcircuit proofs, and that the public `unpruned_hash` values of
/// those circuits hash together to the public `unpruned_hash` value of this
/// circuit.
pub struct BranchSubCircuit<E: Extended = OnlyFull> {
    pub targets: BranchTargets<E>,
    pub indices: PublicIndices,
}

impl<E: Extended> BranchTargets<E> {
    #[must_use]
    pub fn build(self, child: &PublicIndices, public_inputs: &[Target]) -> BranchSubCircuit<E> {
        let indices = PublicIndices {
            unpruned_hash: HashTargetIndex::new(public_inputs, self.inputs.unpruned_hash),
        };
        debug_assert_eq!(indices, *child);

        BranchSubCircuit {
            indices,
            targets: self,
        }
    }
}

impl BranchSubCircuit<OnlyFull> {
    /// Get ready to generate a proof
    pub fn set_witness<F: RichField>(&self, _inputs: &mut PartialWitness<F>) {}

    /// Get ready to generate a proof
    pub fn set_witness_unsafe<F: RichField>(
        &self,
        inputs: &mut PartialWitness<F>,
        unpruned_hash: HashOut<F>,
    ) {
        inputs.set_hash_target(self.targets.inputs.unpruned_hash, unpruned_hash);
    }
}

impl BranchSubCircuit<PartialAllowed> {
    /// Get ready to generate a proof
    pub fn set_witness<F: RichField>(&self, inputs: &mut PartialWitness<F>, partial: bool) {
        inputs.set_bool_target(self.targets.extension.partial, partial);
    }

    /// Get ready to generate a proof
    pub fn set_witness_unsafe<F: RichField>(
        &self,
        inputs: &mut PartialWitness<F>,
        unpruned_hash: HashOut<F>,
        partial: bool,
    ) {
        inputs.set_hash_target(self.targets.inputs.unpruned_hash, unpruned_hash);
        inputs.set_bool_target(self.targets.extension.partial, partial);
    }
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use array_util::ArrayExt;
    use plonky2::plonk::circuit_data::{CircuitConfig, CircuitData};
    use plonky2::plonk::proof::ProofWithPublicInputs;

    use super::*;
    use crate::subcircuits::bounded;
    use crate::test_utils::{
        self, hash_branch, hash_branch_bytes, make_hashes, C, CONFIG, D, F, ZERO_HASH,
    };

    const LEAF_VALUES: usize = 2;
    const NON_ZERO_VALUES: [HashOut<F>; LEAF_VALUES] = make_hashes(test_utils::NON_ZERO_VALUES);

    pub struct DummyLeafCircuit {
        pub bounded: bounded::LeafSubCircuit,
        pub unpruned: LeafSubCircuit,
        pub circuit: CircuitData<F, C, D>,
    }

    impl DummyLeafCircuit {
        #[must_use]
        pub fn new(circuit_config: &CircuitConfig) -> Self {
            let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());

            let bounded_inputs = bounded::SubCircuitInputs::default(&mut builder);
            let unpruned_inputs = SubCircuitInputs::default(&mut builder);

            let bounded_targets = bounded_inputs.build_leaf(&mut builder);
            let unpruned_targets = unpruned_inputs.build_leaf(&mut builder);

            let circuit = builder.build();

            let public_inputs = &circuit.prover_only.public_inputs;
            let bounded = bounded_targets.build(public_inputs);
            let unpruned = unpruned_targets.build(public_inputs);

            Self {
                bounded,
                unpruned,
                circuit,
            }
        }

        pub fn prove(&self, unpruned_hash: HashOut<F>) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.bounded.set_witness(&mut inputs);
            self.unpruned.set_witness(&mut inputs, unpruned_hash);
            self.circuit.prove(inputs)
        }
    }

    struct DummyBranchCircuit<E: Extended> {
        bounded: bounded::BranchSubCircuit<D>,
        unpruned: BranchSubCircuit<E>,
        circuit: CircuitData<F, C, D>,
        vm_hash: bool,
    }

    trait Branch {
        type RightHash<'a>;
        type RightProof<'a>;
        fn hash(&self, l: &HashOut<F>, r: Self::RightHash<'_>) -> HashOut<F>;
        fn circuit(&self) -> &CircuitData<F, C, D>;

        fn prove(
            &self,
            left_proof: &ProofWithPublicInputs<F, C, D>,
            right_proof: Self::RightProof<'_>,
        ) -> Result<ProofWithPublicInputs<F, C, D>>;
    }

    impl<E: Extended> DummyBranchCircuit<E> {
        #[must_use]
        pub fn new(
            circuit_config: &CircuitConfig,
            indices: &PublicIndices,
            child: &CircuitData<F, C, D>,
            vm_hash: bool,
            build_unpruned: impl FnOnce(
                SubCircuitInputs,
                &mut CircuitBuilder<F, D>,
                &PublicIndices,
                &ProofWithPublicInputsTarget<D>,
                &ProofWithPublicInputsTarget<D>,
                bool,
            ) -> BranchTargets<E>,
        ) -> Self {
            let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());

            let bounded_inputs = bounded::SubCircuitInputs::default(&mut builder);
            let unpruned_inputs = SubCircuitInputs::default(&mut builder);

            let bounded_targets = bounded_inputs.build_branch(&mut builder, child);
            let unpruned_targets = build_unpruned(
                unpruned_inputs,
                &mut builder,
                indices,
                &bounded_targets.left_proof,
                &bounded_targets.right_proof,
                vm_hash,
            );

            let circuit = builder.build();

            let public_inputs = &circuit.prover_only.public_inputs;
            let bounded = bounded_targets.build(public_inputs);
            let unpruned = unpruned_targets.build(indices, public_inputs);

            Self {
                bounded,
                unpruned,
                circuit,
                vm_hash,
            }
        }

        fn hash(&self, l: &HashOut<F>, r: &HashOut<F>) -> HashOut<F> {
            if self.vm_hash {
                hash_branch_bytes(l, r)
            } else {
                hash_branch(l, r)
            }
        }
    }

    impl DummyBranchCircuit<OnlyFull> {
        #[must_use]
        pub fn from_leaf(
            circuit_config: &CircuitConfig,
            leaf: &DummyLeafCircuit,
            vm_hash: bool,
        ) -> Self {
            Self::new(
                circuit_config,
                &leaf.unpruned.indices,
                &leaf.circuit,
                vm_hash,
                SubCircuitInputs::build_branch,
            )
        }

        #[must_use]
        pub fn from_branch(circuit_config: &CircuitConfig, branch: &Self, vm_hash: bool) -> Self {
            Self::new(
                circuit_config,
                &branch.unpruned.indices,
                &branch.circuit,
                vm_hash,
                SubCircuitInputs::build_branch,
            )
        }

        fn prove_unsafe(
            &self,
            unpruned_hash: HashOut<F>,
            left_proof: &ProofWithPublicInputs<F, C, D>,
            right_proof: &ProofWithPublicInputs<F, C, D>,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.bounded
                .set_witness(&mut inputs, left_proof, right_proof);
            self.unpruned.set_witness_unsafe(&mut inputs, unpruned_hash);
            self.circuit.prove(inputs)
        }
    }

    impl Branch for DummyBranchCircuit<OnlyFull> {
        type RightHash<'a> = &'a HashOut<F>;
        type RightProof<'a> = &'a ProofWithPublicInputs<F, C, D>;

        fn hash(&self, l: &HashOut<F>, r: &HashOut<F>) -> HashOut<F> { self.hash(l, r) }

        fn circuit(&self) -> &CircuitData<F, C, D> { &self.circuit }

        fn prove(
            &self,
            left_proof: &ProofWithPublicInputs<F, C, D>,
            right_proof: Self::RightProof<'_>,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.bounded
                .set_witness(&mut inputs, left_proof, right_proof);
            self.unpruned.set_witness(&mut inputs);
            self.circuit.prove(inputs)
        }
    }

    impl DummyBranchCircuit<PartialAllowed> {
        #[must_use]
        pub fn from_leaf(
            circuit_config: &CircuitConfig,
            leaf: &DummyLeafCircuit,
            vm_hash: bool,
        ) -> Self {
            Self::new(
                circuit_config,
                &leaf.unpruned.indices,
                &leaf.circuit,
                vm_hash,
                SubCircuitInputs::build_extended_branch,
            )
        }

        #[must_use]
        pub fn from_branch(circuit_config: &CircuitConfig, branch: &Self, vm_hash: bool) -> Self {
            Self::new(
                circuit_config,
                &branch.unpruned.indices,
                &branch.circuit,
                vm_hash,
                SubCircuitInputs::build_extended_branch,
            )
        }

        fn prove_unsafe(
            &self,
            unpruned_hash: HashOut<F>,
            left_proof: &ProofWithPublicInputs<F, C, D>,
            right_proof: Option<&ProofWithPublicInputs<F, C, D>>,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.bounded
                .set_witness(&mut inputs, left_proof, right_proof.unwrap_or(left_proof));
            self.unpruned
                .set_witness_unsafe(&mut inputs, unpruned_hash, right_proof.is_none());
            self.circuit.prove(inputs)
        }
    }

    impl Branch for DummyBranchCircuit<PartialAllowed> {
        type RightHash<'a> = Option<&'a HashOut<F>>;
        type RightProof<'a> = Option<&'a ProofWithPublicInputs<F, C, D>>;

        fn hash(&self, l: &HashOut<F>, r: Self::RightHash<'_>) -> HashOut<F> {
            if let Some(r) = r {
                self.hash(l, r)
            } else {
                *l
            }
        }

        fn prove(
            &self,
            left_proof: &ProofWithPublicInputs<F, C, D>,
            right_proof: Self::RightProof<'_>,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.bounded
                .set_witness(&mut inputs, left_proof, right_proof.unwrap_or(left_proof));
            self.unpruned
                .set_witness(&mut inputs, right_proof.is_none());
            self.circuit.prove(inputs)
        }

        fn circuit(&self) -> &CircuitData<F, C, D> { &self.circuit }
    }

    #[tested_fixture::tested_fixture(LEAF)]
    fn build_leaf() -> DummyLeafCircuit { DummyLeafCircuit::new(&CONFIG) }

    #[tested_fixture::tested_fixture(BRANCH_1)]
    fn build_branch_1() -> DummyBranchCircuit<OnlyFull> {
        DummyBranchCircuit::<OnlyFull>::from_leaf(&CONFIG, &LEAF, false)
    }

    #[tested_fixture::tested_fixture(BRANCH_2)]
    fn build_branch_2() -> DummyBranchCircuit<OnlyFull> {
        DummyBranchCircuit::<OnlyFull>::from_branch(&CONFIG, &BRANCH_1, false)
    }

    #[tested_fixture::tested_fixture(VM_BRANCH_1)]
    fn build_vm_branch_1() -> DummyBranchCircuit<OnlyFull> {
        DummyBranchCircuit::<OnlyFull>::from_leaf(&CONFIG, &LEAF, true)
    }

    #[tested_fixture::tested_fixture(VM_BRANCH_2)]
    fn build_vm_branch_2() -> DummyBranchCircuit<OnlyFull> {
        DummyBranchCircuit::<OnlyFull>::from_branch(&CONFIG, &VM_BRANCH_1, true)
    }

    #[tested_fixture::tested_fixture(PAR_BRANCH_1)]
    fn build_par_branch_1() -> DummyBranchCircuit<PartialAllowed> {
        DummyBranchCircuit::<PartialAllowed>::from_leaf(&CONFIG, &LEAF, false)
    }

    #[tested_fixture::tested_fixture(PAR_BRANCH_2)]
    fn build_par_branch_2() -> DummyBranchCircuit<PartialAllowed> {
        DummyBranchCircuit::<PartialAllowed>::from_branch(&CONFIG, &PAR_BRANCH_1, false)
    }

    fn assert_value(proof: &ProofWithPublicInputs<F, C, D>, hash: HashOut<F>) {
        let indices = &LEAF.unpruned.indices;
        let p_hash = indices.unpruned_hash.get_any(&proof.public_inputs);
        assert_eq!(p_hash, hash.elements);
    }

    fn verify_branch_helper<'a, B: Branch>(
        branch: &B,
        l_hash: &HashOut<F>,
        l_proof: &ProofWithPublicInputs<F, C, D>,
        r_hash: B::RightHash<'a>,
        r_proof: B::RightProof<'a>,
    ) -> Result<(HashOut<F>, ProofWithPublicInputs<F, C, D>)> {
        let hash = branch.hash(l_hash, r_hash);
        let proof = branch.prove(l_proof, r_proof)?;
        assert_value(&proof, hash);
        branch.circuit().verify(proof.clone())?;
        Ok((hash, proof))
    }

    #[tested_fixture::tested_fixture(ZERO_LEAF_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_zero_leaf() -> Result<ProofWithPublicInputs<F, C, D>> {
        let proof = LEAF.prove(ZERO_HASH)?;
        assert_value(&proof, ZERO_HASH);
        LEAF.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(NON_ZERO_LEAF_PROOFS: [(HashOut<F>, ProofWithPublicInputs<F, C, D>); LEAF_VALUES])]
    fn verify_leaf() -> Result<[(HashOut<F>, ProofWithPublicInputs<F, C, D>); LEAF_VALUES]> {
        NON_ZERO_VALUES.try_map_ext(|non_zero_hash| {
            let proof = LEAF.prove(non_zero_hash)?;
            assert_value(&proof, non_zero_hash);
            LEAF.circuit.verify(proof.clone())?;
            Ok((non_zero_hash, proof))
        })
    }

    #[tested_fixture::tested_fixture(ZERO_BRANCH_PROOF: (HashOut<F>, ProofWithPublicInputs<F, C, D>))]
    fn verify_zero_branch() -> Result<(HashOut<F>, ProofWithPublicInputs<F, C, D>)> {
        verify_branch_helper(
            *BRANCH_1,
            &ZERO_HASH,
            *ZERO_LEAF_PROOF,
            &ZERO_HASH,
            *ZERO_LEAF_PROOF,
        )
    }

    #[tested_fixture::tested_fixture(LEFT_BRANCH_PROOFS: [(HashOut<F>, ProofWithPublicInputs<F, C, D>); LEAF_VALUES])]
    fn verify_left_branch() -> Result<[(HashOut<F>, ProofWithPublicInputs<F, C, D>); LEAF_VALUES]> {
        NON_ZERO_LEAF_PROOFS
            .each_ref()
            .try_map_ext(|(non_zero_hash, non_zero_leaf)| {
                verify_branch_helper(
                    *BRANCH_1,
                    non_zero_hash,
                    non_zero_leaf,
                    &ZERO_HASH,
                    &ZERO_LEAF_PROOF,
                )
            })
    }

    #[tested_fixture::tested_fixture(RIGHT_BRANCH_PROOFS: [(HashOut<F>, ProofWithPublicInputs<F, C, D>); LEAF_VALUES])]
    fn verify_right_branch() -> Result<[(HashOut<F>, ProofWithPublicInputs<F, C, D>); LEAF_VALUES]>
    {
        NON_ZERO_LEAF_PROOFS
            .each_ref()
            .try_map_ext(|(non_zero_hash, non_zero_leaf)| {
                verify_branch_helper(
                    *BRANCH_1,
                    &ZERO_HASH,
                    &ZERO_LEAF_PROOF,
                    non_zero_hash,
                    non_zero_leaf,
                )
            })
    }

    #[tested_fixture::tested_fixture(FULL_BRANCH_PROOFS: [[(HashOut<F>, ProofWithPublicInputs<F, C, D>); LEAF_VALUES]; LEAF_VALUES])]
    fn verify_full_branch(
    ) -> Result<[[(HashOut<F>, ProofWithPublicInputs<F, C, D>); LEAF_VALUES]; LEAF_VALUES]> {
        let leaf_proofs = NON_ZERO_LEAF_PROOFS.each_ref();
        leaf_proofs.try_map_ext(|(non_zero_hash_1, non_zero_leaf_1)| {
            leaf_proofs.try_map_ext(|(non_zero_hash_2, non_zero_leaf_2)| {
                verify_branch_helper(
                    *BRANCH_1,
                    non_zero_hash_1,
                    non_zero_leaf_1,
                    non_zero_hash_2,
                    non_zero_leaf_2,
                )
            })
        })
    }

    #[test]
    fn verify_double_branch() -> Result<()> {
        let branches = chain![
            [*ZERO_BRANCH_PROOF],
            LEFT_BRANCH_PROOFS.iter().take(1),
            RIGHT_BRANCH_PROOFS.iter().take(1),
            FULL_BRANCH_PROOFS.iter().flat_map(|v| v.iter().take(1)),
        ];

        for (ref hash_1, ref proof_1) in branches.clone() {
            for (ref hash_2, ref proof_2) in branches.clone() {
                verify_branch_helper(*BRANCH_2, hash_1, proof_1, hash_2, proof_2)?;
                verify_branch_helper(*BRANCH_2, hash_2, proof_2, hash_1, proof_1)?;
            }
        }

        Ok(())
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_wrong_hash_branch() {
        let branch_proof = BRANCH_1
            .prove_unsafe(
                NON_ZERO_LEAF_PROOFS[0].0,
                &NON_ZERO_LEAF_PROOFS[0].1,
                &NON_ZERO_LEAF_PROOFS[1].1,
            )
            .unwrap();
        BRANCH_1.circuit.verify(branch_proof).unwrap();
    }

    #[tested_fixture::tested_fixture(ZERO_VM_BRANCH_PROOF: (HashOut<F>, ProofWithPublicInputs<F, C, D>))]
    fn verify_zero_vm_branch() -> Result<(HashOut<F>, ProofWithPublicInputs<F, C, D>)> {
        verify_branch_helper(
            *VM_BRANCH_1,
            &ZERO_HASH,
            *ZERO_LEAF_PROOF,
            &ZERO_HASH,
            *ZERO_LEAF_PROOF,
        )
    }

    #[tested_fixture::tested_fixture(LEFT_VM_BRANCH_PROOFS: [(HashOut<F>, ProofWithPublicInputs<F, C, D>); LEAF_VALUES])]
    fn verify_left_vm_branch() -> Result<[(HashOut<F>, ProofWithPublicInputs<F, C, D>); LEAF_VALUES]>
    {
        NON_ZERO_LEAF_PROOFS
            .each_ref()
            .try_map_ext(|(non_zero_hash, non_zero_leaf)| {
                verify_branch_helper(
                    *VM_BRANCH_1,
                    non_zero_hash,
                    non_zero_leaf,
                    &ZERO_HASH,
                    &ZERO_LEAF_PROOF,
                )
            })
    }

    #[tested_fixture::tested_fixture(RIGHT_VM_BRANCH_PROOFS: [(HashOut<F>, ProofWithPublicInputs<F, C, D>); LEAF_VALUES])]
    fn verify_right_vm_branch(
    ) -> Result<[(HashOut<F>, ProofWithPublicInputs<F, C, D>); LEAF_VALUES]> {
        NON_ZERO_LEAF_PROOFS
            .each_ref()
            .try_map_ext(|(non_zero_hash, non_zero_leaf)| {
                verify_branch_helper(
                    *VM_BRANCH_1,
                    &ZERO_HASH,
                    &ZERO_LEAF_PROOF,
                    non_zero_hash,
                    non_zero_leaf,
                )
            })
    }

    #[tested_fixture::tested_fixture(FULL_VM_BRANCH_PROOFS: [[(HashOut<F>, ProofWithPublicInputs<F, C, D>); LEAF_VALUES]; LEAF_VALUES])]
    fn verify_full_vm_branch(
    ) -> Result<[[(HashOut<F>, ProofWithPublicInputs<F, C, D>); LEAF_VALUES]; LEAF_VALUES]> {
        let leaf_proofs = NON_ZERO_LEAF_PROOFS.each_ref();
        leaf_proofs.try_map_ext(|(non_zero_hash_1, non_zero_leaf_1)| {
            leaf_proofs.try_map_ext(|(non_zero_hash_2, non_zero_leaf_2)| {
                verify_branch_helper(
                    *VM_BRANCH_1,
                    non_zero_hash_1,
                    non_zero_leaf_1,
                    non_zero_hash_2,
                    non_zero_leaf_2,
                )
            })
        })
    }

    #[test]
    fn verify_double_vm_branch() -> Result<()> {
        let branches = chain![
            [*ZERO_VM_BRANCH_PROOF],
            LEFT_VM_BRANCH_PROOFS.iter().take(1),
            RIGHT_VM_BRANCH_PROOFS.iter().take(1),
            FULL_VM_BRANCH_PROOFS.iter().flat_map(|v| v.iter().take(1)),
        ];

        for (ref hash_1, ref proof_1) in branches.clone() {
            for (ref hash_2, ref proof_2) in branches.clone() {
                verify_branch_helper(*VM_BRANCH_2, hash_1, proof_1, hash_2, proof_2)?;
                verify_branch_helper(*VM_BRANCH_2, hash_2, proof_2, hash_1, proof_1)?;
            }
        }

        Ok(())
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_wrong_hash_vm_branch() {
        let branch_proof = VM_BRANCH_1
            .prove_unsafe(
                NON_ZERO_LEAF_PROOFS[0].0,
                &NON_ZERO_LEAF_PROOFS[0].1,
                &NON_ZERO_LEAF_PROOFS[1].1,
            )
            .unwrap();
        VM_BRANCH_1.circuit.verify(branch_proof).unwrap();
    }

    #[tested_fixture::tested_fixture(ZERO_PAR_BRANCH_PROOF: [(HashOut<F>, ProofWithPublicInputs<F, C, D>); 2])]
    fn verify_zero_partial_branch() -> Result<[(HashOut<F>, ProofWithPublicInputs<F, C, D>); 2]> {
        Ok([
            verify_branch_helper(*PAR_BRANCH_1, &ZERO_HASH, *ZERO_LEAF_PROOF, None, None)?,
            verify_branch_helper(
                *PAR_BRANCH_1,
                &ZERO_HASH,
                *ZERO_LEAF_PROOF,
                Some(&ZERO_HASH),
                Some(*ZERO_LEAF_PROOF),
            )?,
        ])
    }

    #[tested_fixture::tested_fixture(LEFT_PAR_BRANCH_PROOFS: [[(HashOut<F>, ProofWithPublicInputs<F, C, D>); 2]; LEAF_VALUES])]
    fn verify_left_partial_branch(
    ) -> Result<[[(HashOut<F>, ProofWithPublicInputs<F, C, D>); 2]; LEAF_VALUES]> {
        NON_ZERO_LEAF_PROOFS
            .each_ref()
            .try_map_ext(|(non_zero_hash, non_zero_leaf)| {
                Ok([
                    verify_branch_helper(*PAR_BRANCH_1, non_zero_hash, non_zero_leaf, None, None)?,
                    verify_branch_helper(
                        *PAR_BRANCH_1,
                        non_zero_hash,
                        non_zero_leaf,
                        Some(&ZERO_HASH),
                        Some(*ZERO_LEAF_PROOF),
                    )?,
                ])
            })
    }

    #[tested_fixture::tested_fixture(RIGHT_PAR_BRANCH_PROOFS: [(HashOut<F>, ProofWithPublicInputs<F, C, D>); LEAF_VALUES])]
    fn verify_right_partial_branch(
    ) -> Result<[(HashOut<F>, ProofWithPublicInputs<F, C, D>); LEAF_VALUES]> {
        NON_ZERO_LEAF_PROOFS
            .each_ref()
            .try_map_ext(|(non_zero_hash, non_zero_leaf)| {
                verify_branch_helper(
                    *PAR_BRANCH_1,
                    &ZERO_HASH,
                    &ZERO_LEAF_PROOF,
                    Some(non_zero_hash),
                    Some(non_zero_leaf),
                )
            })
    }

    #[tested_fixture::tested_fixture(FULL_PAR_BRANCH_PROOFS: [[(HashOut<F>, ProofWithPublicInputs<F, C, D>); LEAF_VALUES]; LEAF_VALUES])]
    fn verify_full_partial_branch(
    ) -> Result<[[(HashOut<F>, ProofWithPublicInputs<F, C, D>); LEAF_VALUES]; LEAF_VALUES]> {
        let leaf_proofs = NON_ZERO_LEAF_PROOFS.each_ref();
        leaf_proofs.try_map_ext(|(non_zero_hash_1, non_zero_leaf_1)| {
            leaf_proofs.try_map_ext(|(non_zero_hash_2, non_zero_leaf_2)| {
                verify_branch_helper(
                    *PAR_BRANCH_1,
                    non_zero_hash_1,
                    non_zero_leaf_1,
                    Some(non_zero_hash_2),
                    Some(non_zero_leaf_2),
                )
            })
        })
    }

    #[test]
    fn verify_double_partial_branch() -> Result<()> {
        let branches = chain![
            ZERO_PAR_BRANCH_PROOF.iter(),
            LEFT_PAR_BRANCH_PROOFS.iter().flat_map(|v| v.iter().take(1)),
            RIGHT_PAR_BRANCH_PROOFS.iter(),
            FULL_PAR_BRANCH_PROOFS.iter().flat_map(|v| v.iter().take(1)),
        ];

        for (ref hash_1, ref proof_1) in branches.clone() {
            verify_branch_helper(*PAR_BRANCH_2, hash_1, proof_1, None, None)?;
            for (ref hash_2, ref proof_2) in branches.clone() {
                verify_branch_helper(*PAR_BRANCH_2, hash_1, proof_1, Some(hash_2), Some(proof_2))?;
                verify_branch_helper(*PAR_BRANCH_2, hash_2, proof_2, Some(hash_1), Some(proof_1))?;
            }
        }

        Ok(())
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_wrong_hash_partial_branch_1() {
        let branch_proof = PAR_BRANCH_1
            .prove_unsafe(
                NON_ZERO_LEAF_PROOFS[0].0,
                &NON_ZERO_LEAF_PROOFS[0].1,
                Some(&NON_ZERO_LEAF_PROOFS[1].1),
            )
            .unwrap();
        PAR_BRANCH_1.circuit.verify(branch_proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_wrong_hash_partial_branch_2() {
        let branch_proof = PAR_BRANCH_1
            .prove_unsafe(NON_ZERO_LEAF_PROOFS[0].0, &NON_ZERO_LEAF_PROOFS[1].1, None)
            .unwrap();
        PAR_BRANCH_1.circuit.verify(branch_proof).unwrap();
    }
}

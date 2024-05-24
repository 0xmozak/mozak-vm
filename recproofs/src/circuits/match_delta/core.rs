//! Subcircuits for recursively proving state delta objects match summarized
//! state updates

use enumflags2::BitFlags;
use itertools::chain;
use plonky2::field::extension::Extendable;
use plonky2::field::types::Field;
use plonky2::hash::hash_types::{HashOut, HashOutTarget, RichField};
use plonky2::hash::poseidon2::Poseidon2Hash;
use plonky2::iop::target::Target;
use plonky2::iop::witness::{PartialWitness, WitnessWrite};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::proof::ProofWithPublicInputsTarget;

use crate::{are_equal, are_zero, zero_if, EventFlags};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct PublicIndices {}

pub struct SubCircuitInputs {}

pub struct LeafTargets {
    /// The public inputs
    pub inputs: SubCircuitInputs,

    /// The object/event block height
    ///
    /// This is also the new `last_updated` for any writes
    pub block_height: Target,

    /// The object address
    pub address: Target,

    /// The (partial) object flags
    pub object_flags: Target,

    /// The previous constraint owner
    pub old_owner: [Target; 4],

    /// The new constraint owner
    pub new_owner: [Target; 4],

    /// The previous data
    pub old_data: [Target; 4],

    /// The new data
    pub new_data: [Target; 4],

    /// The old `last_updated`
    ///
    /// This is also the "new" `last_updated` for any read-only changes
    pub last_updated: Target,

    /// The old credits
    pub old_credits: Target,

    /// The new credits
    pub new_credits: Target,

    /// The state hash
    pub state_hash: HashOutTarget,
}

impl SubCircuitInputs {
    #[must_use]
    pub fn default<F, const D: usize>(_builder: &mut CircuitBuilder<F, D>) -> Self
    where
        F: RichField + Extendable<D>, {
        Self {}
    }

    #[must_use]
    pub fn build_leaf<F, const D: usize>(self, builder: &mut CircuitBuilder<F, D>) -> LeafTargets
    where
        F: RichField + Extendable<D>, {
        let one = builder.one();

        let block_height = builder.add_virtual_target();
        let address = builder.add_virtual_target();
        let object_flags = builder.add_virtual_target();
        let old_owner = [(); 4].map(|()| builder.add_virtual_target());
        let new_owner = [(); 4].map(|()| builder.add_virtual_target());
        let old_data = [(); 4].map(|()| builder.add_virtual_target());
        let new_data = [(); 4].map(|()| builder.add_virtual_target());
        let last_updated = builder.add_virtual_target();
        let old_credits = builder.add_virtual_target();
        let new_credits = builder.add_virtual_target();

        let (write_flag, give_owner_flag, take_owner_flag) = {
            let object_flags = builder.split_le(object_flags, EventFlags::count());

            (
                object_flags[EventFlags::WriteFlag.index()],
                object_flags[EventFlags::GiveOwnerFlag.index()],
                object_flags[EventFlags::TakeOwnerFlag.index()],
            )
        };

        let credits_unchanged = builder.is_equal(old_credits, new_credits);
        let credits_updated = builder.not(credits_unchanged);

        // Require ownership to be complete
        builder.connect(give_owner_flag.target, take_owner_flag.target);

        // Require writes to be complete
        let data_unchanged = are_equal(builder, old_data, new_data);
        let unchanged_or_flag = builder.or(data_unchanged, write_flag);
        builder.connect(unchanged_or_flag.target, one);

        // Ensure ownership changes are limited to creation and deletion
        let no_owner_change = are_equal(builder, old_owner, new_owner);
        let is_creation = are_zero(builder, old_owner);
        let is_deletion = are_zero(builder, new_owner);
        let is_owner_change = builder.add(is_creation.target, is_deletion.target);
        let ownership_mode = builder.add(is_owner_change, no_owner_change.target);
        builder.connect(ownership_mode, one); // Exactly one mode is selected
        builder.connect(is_owner_change, give_owner_flag.target); // Mode based on flags

        // Use block_height for any writes
        let updated = builder.or(give_owner_flag, write_flag);
        let updated = builder.or(updated, credits_updated);
        let new_last_updated = builder.select(updated, block_height, last_updated);
        let new_last_updated = zero_if(builder, is_deletion, new_last_updated);

        // Build the old hash
        let old_hash = chain!(old_owner, [last_updated, old_credits], old_data).collect();
        let old_hash = builder.hash_n_to_hash_no_pad::<Poseidon2Hash>(old_hash);

        // Build the new hash
        let new_hash = chain!(new_owner, [new_last_updated, new_credits], new_data).collect();
        let new_hash = builder.hash_n_to_hash_no_pad::<Poseidon2Hash>(new_hash);

        // Build the state summary hash
        let state_hash = chain!([address], old_hash.elements, new_hash.elements).collect();
        let state_hash = builder.hash_n_to_hash_no_pad::<Poseidon2Hash>(state_hash);

        LeafTargets {
            inputs: self,

            block_height,
            address,
            object_flags,
            old_owner,
            new_owner,
            old_data,
            new_data,
            last_updated,
            old_credits,
            new_credits,
            state_hash,
        }
    }
}

/// The leaf subcircuit metadata. This subcircuit validates a partial
/// object corresponds to a state update.
pub struct LeafSubCircuit {
    pub targets: LeafTargets,
    pub indices: PublicIndices,
}

impl LeafTargets {
    #[must_use]
    pub fn build(self, _public_inputs: &[Target]) -> LeafSubCircuit {
        // Find the indices
        let indices = PublicIndices {};
        LeafSubCircuit {
            targets: self,
            indices,
        }
    }
}

#[derive(Clone, Copy)]
pub struct LeafWitnessValue<F: Field> {
    /// The object/event block height
    ///
    /// This is also the new `last_updated` for any writes
    pub block_height: u64,

    /// The object address
    pub address: u64,

    /// The (partial) object flags
    pub object_flags: BitFlags<EventFlags>,

    /// The previous constraint owner
    pub old_owner: [F; 4],

    /// The new constraint owner
    pub new_owner: [F; 4],

    /// The previous data
    pub old_data: [F; 4],

    /// The new data
    pub new_data: [F; 4],

    /// The old `last_updated`
    ///
    /// This is also the "new" `last_updated` for any read-only changes
    pub last_updated: u64,

    /// The old credits
    pub old_credits: u64,

    /// The new credits
    pub new_credits: u64,
}

impl LeafSubCircuit {
    /// Get ready to generate a proof
    pub fn set_witness<F: RichField>(
        &self,
        witness: &mut PartialWitness<F>,
        v: LeafWitnessValue<F>,
    ) {
        self.set_witness_unsafe(witness, v, None);
    }

    pub fn set_witness_unsafe<F: RichField>(
        &self,
        inputs: &mut PartialWitness<F>,
        v: LeafWitnessValue<F>,
        state_hash: Option<HashOut<F>>,
    ) {
        inputs.set_target(
            self.targets.block_height,
            F::from_canonical_u64(v.block_height),
        );
        inputs.set_target(self.targets.address, F::from_canonical_u64(v.address));
        inputs.set_target(
            self.targets.object_flags,
            F::from_canonical_u8(v.object_flags.bits()),
        );
        inputs.set_target_arr(&self.targets.old_owner, &v.old_owner);
        inputs.set_target_arr(&self.targets.new_owner, &v.new_owner);
        inputs.set_target_arr(&self.targets.old_data, &v.old_data);
        inputs.set_target_arr(&self.targets.new_data, &v.new_data);
        inputs.set_target(
            self.targets.last_updated,
            F::from_canonical_u64(v.last_updated),
        );
        inputs.set_target(
            self.targets.old_credits,
            F::from_canonical_u64(v.old_credits),
        );
        inputs.set_target(
            self.targets.new_credits,
            F::from_canonical_u64(v.new_credits),
        );
        if let Some(state_hash) = state_hash {
            inputs.set_hash_target(self.targets.state_hash, state_hash);
        }
    }
}

pub struct BranchTargets {
    /// The public inputs
    pub inputs: SubCircuitInputs,

    /// The left direction
    pub left: SubCircuitInputs,

    /// The right direction
    pub right: SubCircuitInputs,
}

impl SubCircuitInputs {
    #[allow(clippy::trivially_copy_pass_by_ref)]
    fn direction_from_node<const D: usize>(
        _proof: &ProofWithPublicInputsTarget<D>,
        _indices: &PublicIndices,
    ) -> SubCircuitInputs {
        SubCircuitInputs {}
    }

    #[must_use]
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn build_branch<F: RichField + Extendable<D>, const D: usize>(
        self,
        _builder: &mut CircuitBuilder<F, D>,
        indices: &PublicIndices,
        left_proof: &ProofWithPublicInputsTarget<D>,
        right_proof: &ProofWithPublicInputsTarget<D>,
    ) -> BranchTargets {
        let left = Self::direction_from_node(left_proof, indices);
        let right = Self::direction_from_node(right_proof, indices);

        BranchTargets {
            inputs: self,
            left,
            right,
        }
    }
}

pub struct BranchSubCircuit {
    pub targets: BranchTargets,
    pub indices: PublicIndices,
}

impl BranchTargets {
    #[must_use]
    pub fn build(self, child: &PublicIndices, _public_inputs: &[Target]) -> BranchSubCircuit {
        let indices = PublicIndices {};
        debug_assert_eq!(indices, *child);

        BranchSubCircuit {
            indices,
            targets: self,
        }
    }
}

impl BranchSubCircuit {
    pub fn set_witness<F: RichField>(&self, _inputs: &mut PartialWitness<F>) {}
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use plonky2::plonk::circuit_data::{CircuitConfig, CircuitData};
    use plonky2::plonk::config::Hasher;
    use plonky2::plonk::proof::ProofWithPublicInputs;

    use super::*;
    use crate::subcircuits::bounded;
    use crate::test_utils::{C, CONFIG, D, F};

    pub struct DummyLeafCircuit {
        pub bounded: bounded::LeafSubCircuit,
        pub compare_object: LeafSubCircuit,
        pub circuit: CircuitData<F, C, D>,
    }

    impl DummyLeafCircuit {
        #[must_use]
        pub fn new(circuit_config: &CircuitConfig) -> Self {
            let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());

            let bounded_inputs = bounded::SubCircuitInputs::default(&mut builder);
            let compare_object_inputs = SubCircuitInputs::default(&mut builder);

            let bounded_targets = bounded_inputs.build_leaf(&mut builder);
            let compare_object_targets = compare_object_inputs.build_leaf(&mut builder);

            let circuit = builder.build();

            let public_inputs = &circuit.prover_only.public_inputs;
            let bounded = bounded_targets.build(public_inputs);
            let compare_object = compare_object_targets.build(public_inputs);

            Self {
                bounded,
                compare_object,
                circuit,
            }
        }

        pub fn prove(&self, v: LeafWitnessValue<F>) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.bounded.set_witness(&mut inputs);
            self.compare_object.set_witness(&mut inputs, v);
            self.circuit.prove(inputs)
        }

        fn prove_unsafe(
            &self,
            v: LeafWitnessValue<F>,
            state_hash: HashOut<F>,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.bounded.set_witness(&mut inputs);
            self.compare_object
                .set_witness_unsafe(&mut inputs, v, Some(state_hash));
            self.circuit.prove(inputs)
        }
    }

    pub struct DummyBranchCircuit {
        pub bounded: bounded::BranchSubCircuit<D>,
        pub compare_object: BranchSubCircuit,
        pub circuit: CircuitData<F, C, D>,
    }

    impl DummyBranchCircuit {
        #[must_use]
        #[allow(clippy::trivially_copy_pass_by_ref)]
        pub fn new(
            circuit_config: &CircuitConfig,
            indices: &PublicIndices,
            child: &CircuitData<F, C, D>,
        ) -> Self {
            let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());

            let bounded_inputs = bounded::SubCircuitInputs::default(&mut builder);
            let compare_object_inputs = SubCircuitInputs::default(&mut builder);

            let bounded_targets = bounded_inputs.build_branch(&mut builder, child);
            let compare_object_targets = compare_object_inputs.build_branch(
                &mut builder,
                indices,
                &bounded_targets.left_proof,
                &bounded_targets.right_proof,
            );

            let circuit = builder.build();

            let public_inputs = &circuit.prover_only.public_inputs;
            let bounded = bounded_targets.build(public_inputs);
            let compare_object = compare_object_targets.build(indices, public_inputs);

            Self {
                bounded,
                compare_object,
                circuit,
            }
        }

        #[must_use]
        pub fn from_leaf(circuit_config: &CircuitConfig, leaf: &DummyLeafCircuit) -> Self {
            Self::new(circuit_config, &leaf.compare_object.indices, &leaf.circuit)
        }

        #[must_use]
        pub fn from_branch(circuit_config: &CircuitConfig, branch: &Self) -> Self {
            Self::new(
                circuit_config,
                &branch.compare_object.indices,
                &branch.circuit,
            )
        }

        pub fn prove(
            &self,
            left_proof: &ProofWithPublicInputs<F, C, D>,
            right_proof: &ProofWithPublicInputs<F, C, D>,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.bounded
                .set_witness(&mut inputs, left_proof, right_proof);
            self.compare_object.set_witness(&mut inputs);
            self.circuit.prove(inputs)
        }
    }

    #[tested_fixture::tested_fixture(LEAF)]
    fn build_leaf() -> DummyLeafCircuit { DummyLeafCircuit::new(&CONFIG) }

    #[tested_fixture::tested_fixture(BRANCH_1)]
    fn build_branch_1() -> DummyBranchCircuit { DummyBranchCircuit::from_leaf(&CONFIG, &LEAF) }

    #[tested_fixture::tested_fixture(BRANCH_2)]
    fn build_branch_2() -> DummyBranchCircuit {
        DummyBranchCircuit::from_branch(&CONFIG, &BRANCH_1)
    }

    #[test]
    fn verify_leaf() -> Result<()> {
        let zero_val = [F::ZERO; 4];
        let program_hash_1 = [4, 8, 15, 16].map(F::from_canonical_u64);
        let non_zero_val_1 = [3, 1, 4, 15].map(F::from_canonical_u64);
        let non_zero_val_2 = [42, 0, 0, 0].map(F::from_canonical_u64);

        let creation = LeafWitnessValue {
            address: 42,
            block_height: 23,
            object_flags: EventFlags::GiveOwnerFlag
                | EventFlags::TakeOwnerFlag
                | EventFlags::WriteFlag,
            old_owner: zero_val,
            new_owner: program_hash_1,
            old_data: zero_val,
            new_data: non_zero_val_1,
            last_updated: 0,
            old_credits: 0,
            new_credits: 150,
        };
        let proof = LEAF.prove(creation)?;
        LEAF.circuit.verify(proof)?;

        let write = LeafWitnessValue {
            block_height: 24,
            object_flags: EventFlags::WriteFlag.into(),
            old_owner: program_hash_1,
            old_data: non_zero_val_1,
            new_data: non_zero_val_2,
            last_updated: 23,
            old_credits: 150,
            ..creation
        };
        let proof = LEAF.prove(write)?;
        LEAF.circuit.verify(proof)?;

        let read = LeafWitnessValue {
            block_height: 25,
            object_flags: EventFlags::ReadFlag | EventFlags::EnsureFlag,
            old_data: non_zero_val_2,
            last_updated: 24,
            ..write
        };
        let proof = LEAF.prove(read)?;
        LEAF.circuit.verify(proof)?;

        let burn = LeafWitnessValue {
            block_height: 26,
            object_flags: BitFlags::EMPTY,
            last_updated: 24,
            new_credits: 130,
            ..read
        };
        let proof = LEAF.prove(burn)?;
        LEAF.circuit.verify(proof)?;

        let mint = LeafWitnessValue {
            block_height: 27,
            object_flags: BitFlags::EMPTY,
            last_updated: 26,
            old_credits: 130,
            new_credits: 190,
            ..burn
        };
        let proof = LEAF.prove(mint)?;
        LEAF.circuit.verify(proof)?;

        let deletion = LeafWitnessValue {
            block_height: 28,
            object_flags: EventFlags::GiveOwnerFlag
                | EventFlags::TakeOwnerFlag
                | EventFlags::WriteFlag,
            new_owner: zero_val,
            new_data: zero_val,
            last_updated: 27,
            old_credits: 190,
            new_credits: 0,
            ..burn
        };
        let proof = LEAF.prove(deletion)?;
        LEAF.circuit.verify(proof)?;

        Ok(())
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_creation_leaf_1() {
        let zero_val = [F::ZERO; 4];
        let program_hash_1 = [4, 8, 15, 16].map(F::from_canonical_u64);
        let non_zero_val_1 = [3, 1, 4, 15].map(F::from_canonical_u64);

        let creation = LeafWitnessValue {
            address: 42,
            block_height: 23,
            object_flags: EventFlags::GiveOwnerFlag.into(),
            old_owner: zero_val,
            new_owner: program_hash_1,
            old_data: zero_val,
            new_data: non_zero_val_1,
            last_updated: 0,
            old_credits: 0,
            new_credits: 150,
        };
        let proof = LEAF.prove(creation).unwrap();
        LEAF.circuit.verify(proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_creation_leaf_2() {
        let zero_val = [F::ZERO; 4];
        let program_hash_1 = [4, 8, 15, 16].map(F::from_canonical_u64);
        let non_zero_val_1 = [3, 1, 4, 15].map(F::from_canonical_u64);

        let creation = LeafWitnessValue {
            address: 42,
            block_height: 23,
            object_flags: EventFlags::TakeOwnerFlag.into(),
            old_owner: zero_val,
            new_owner: program_hash_1,
            old_data: zero_val,
            new_data: non_zero_val_1,
            last_updated: 0,
            old_credits: 0,
            new_credits: 150,
        };
        let proof = LEAF.prove(creation).unwrap();
        LEAF.circuit.verify(proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_creation_leaf_3() {
        let zero_val = [F::ZERO; 4];
        let program_hash_1 = [4, 8, 15, 16].map(F::from_canonical_u64);
        let non_zero_val_1 = [3, 1, 4, 15].map(F::from_canonical_u64);

        let creation = LeafWitnessValue {
            address: 42,
            block_height: 23,
            object_flags: EventFlags::GiveOwnerFlag | EventFlags::TakeOwnerFlag,
            old_owner: program_hash_1,
            new_owner: program_hash_1,
            old_data: zero_val,
            new_data: non_zero_val_1,
            last_updated: 0,
            old_credits: 0,
            new_credits: 150,
        };
        let proof = LEAF.prove(creation).unwrap();
        LEAF.circuit.verify(proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_creation_leaf_4() {
        let zero_val = [F::ZERO; 4];
        let non_zero_val_1 = [3, 1, 4, 15].map(F::from_canonical_u64);

        let creation = LeafWitnessValue {
            address: 42,
            block_height: 23,
            object_flags: EventFlags::GiveOwnerFlag | EventFlags::TakeOwnerFlag,
            old_owner: zero_val,
            new_owner: zero_val,
            old_data: zero_val,
            new_data: non_zero_val_1,
            last_updated: 0,
            old_credits: 0,
            new_credits: 150,
        };
        let proof = LEAF.prove(creation).unwrap();
        LEAF.circuit.verify(proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    /// Only creation and deletion should be allowed
    fn bad_transer_leaf() {
        let program_hash_1 = [4, 8, 15, 16].map(F::from_canonical_u64);
        let program_hash_2 = [2, 3, 4, 2].map(F::from_canonical_u64);
        let non_zero_val_1 = [3, 1, 4, 15].map(F::from_canonical_u64);

        let creation = LeafWitnessValue {
            address: 42,
            block_height: 23,
            object_flags: EventFlags::GiveOwnerFlag | EventFlags::TakeOwnerFlag,
            old_owner: program_hash_1,
            new_owner: program_hash_2,
            old_data: non_zero_val_1,
            new_data: non_zero_val_1,
            last_updated: 0,
            old_credits: 0,
            new_credits: 150,
        };
        let proof = LEAF.prove(creation).unwrap();
        LEAF.circuit.verify(proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    /// Updates should require the `Write` flag to be set

    fn bad_ensure_leaf() {
        let program_hash_1 = [4, 8, 15, 16].map(F::from_canonical_u64);
        let non_zero_val_1 = [3, 1, 4, 15].map(F::from_canonical_u64);
        let non_zero_val_2 = [42, 0, 0, 0].map(F::from_canonical_u64);

        let creation = LeafWitnessValue {
            address: 42,
            block_height: 23,
            object_flags: EventFlags::EnsureFlag.into(),
            old_owner: program_hash_1,
            new_owner: program_hash_1,
            old_data: non_zero_val_1,
            new_data: non_zero_val_2,
            last_updated: 0,
            old_credits: 0,
            new_credits: 0,
        };
        let proof = LEAF.prove(creation).unwrap();
        LEAF.circuit.verify(proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_hash_leaf() {
        let zero_val = [F::ZERO; 4];
        let program_hash_1 = [4, 8, 15, 16].map(F::from_canonical_u64);
        let non_zero_val_1 = [3, 1, 4, 15].map(F::from_canonical_u64);

        let creation = LeafWitnessValue {
            address: 42,
            block_height: 23,
            object_flags: EventFlags::GiveOwnerFlag | EventFlags::TakeOwnerFlag,
            old_owner: zero_val,
            new_owner: program_hash_1,
            old_data: zero_val,
            new_data: non_zero_val_1,
            last_updated: 0,
            old_credits: 0,
            new_credits: 150,
        };
        let new_hash: Vec<F> = chain!(
            creation.new_owner,
            [creation.block_height, creation.new_credits].map(F::from_canonical_u64),
            creation.new_data
        )
        .collect();
        let new_hash = Poseidon2Hash::hash_no_pad(&new_hash);
        let state_hash: Vec<F> = chain!(
            [F::from_canonical_u64(creation.address - 1)],
            zero_val,
            new_hash.elements
        )
        .collect();
        let state_hash = Poseidon2Hash::hash_no_pad(&state_hash);
        let proof = LEAF.prove_unsafe(creation, state_hash).unwrap();
        LEAF.circuit.verify(proof).unwrap();
    }

    #[test]
    fn verify_branch() -> Result<()> {
        let zero_val = [F::ZERO; 4];
        let program_hash_1 = [4, 8, 15, 16].map(F::from_canonical_u64);
        let program_hash_2 = [2, 3, 4, 2].map(F::from_canonical_u64);
        let non_zero_val_1 = [3, 1, 4, 15].map(F::from_canonical_u64);
        let non_zero_val_2 = [42, 0, 0, 0].map(F::from_canonical_u64);

        let creation_1 = LeafWitnessValue {
            address: 42,
            block_height: 23,
            object_flags: EventFlags::GiveOwnerFlag
                | EventFlags::TakeOwnerFlag
                | EventFlags::WriteFlag,
            old_owner: zero_val,
            new_owner: program_hash_1,
            old_data: zero_val,
            new_data: non_zero_val_1,
            last_updated: 0,
            old_credits: 0,
            new_credits: 150,
        };
        let creation_1 = LEAF.prove(creation_1)?;
        LEAF.circuit.verify(creation_1.clone())?;

        let creation_2 = LeafWitnessValue {
            address: 142,
            block_height: 123,
            object_flags: EventFlags::GiveOwnerFlag
                | EventFlags::TakeOwnerFlag
                | EventFlags::WriteFlag,
            old_owner: zero_val,
            new_owner: program_hash_2,
            old_data: zero_val,
            new_data: non_zero_val_2,
            last_updated: 0,
            old_credits: 0,
            new_credits: 88,
        };
        let creation_2 = LEAF.prove(creation_2)?;
        LEAF.circuit.verify(creation_2.clone())?;

        let branch_proof_1 = BRANCH_1.prove(&creation_1, &creation_2)?;
        BRANCH_1.circuit.verify(branch_proof_1.clone())?;

        let branch_proof_2 = BRANCH_1.prove(&creation_2, &creation_1)?;
        BRANCH_1.circuit.verify(branch_proof_2.clone())?;

        let double_branch_proof = BRANCH_2.prove(&branch_proof_1, &branch_proof_2)?;
        BRANCH_2.circuit.verify(double_branch_proof)?;

        let double_branch_proof = BRANCH_2.prove(&branch_proof_2, &branch_proof_1)?;
        BRANCH_2.circuit.verify(double_branch_proof)?;

        Ok(())
    }
}

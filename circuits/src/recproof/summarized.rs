//! Subcircuits for recursively proving partial contents of a merkle tree
//!
//! These subcircuits are pseudo-recursive, building on top of each other to
//! create the next level up of the merkle tree. "Pseudo-" here means the height
//! must be fixed ahead of time and not depend on the content.
//!
//! These subcircuits are useful to prove knowledge of a selected subset of
//! nodes.
use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::{HashOut, HashOutTarget, RichField, NUM_HASH_OUT_ELTS};
use plonky2::iop::target::{BoolTarget, Target};
use plonky2::iop::witness::{PartialWitness, WitnessWrite};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::proof::ProofWithPublicInputsTarget;

use super::{find_bool, find_hash, hash_or_forward};

#[derive(Copy, Clone)]
pub struct PublicIndices {
    pub summary_hash_present: usize,
    pub summary_hash: [usize; NUM_HASH_OUT_ELTS],
}

impl PublicIndices {
    pub fn get_summary_hash_present<T: Copy>(&self, public_inputs: &[T]) -> T {
        public_inputs[self.summary_hash_present]
    }

    pub fn get_summary_hash<T: Copy>(&self, public_inputs: &[T]) -> [T; NUM_HASH_OUT_ELTS] {
        self.summary_hash.map(|i| public_inputs[i])
    }

    pub fn set_summary_hash_present<T>(&self, public_inputs: &mut [T], v: T) {
        public_inputs[self.summary_hash_present] = v;
    }

    pub fn set_summary_hash<T>(&self, public_inputs: &mut [T], v: [T; NUM_HASH_OUT_ELTS]) {
        for (i, v) in v.into_iter().enumerate() {
            public_inputs[self.summary_hash[i]] = v;
        }
    }
}

pub struct SubCircuitInputs {
    pub summary_hash_present: BoolTarget,

    /// The hash of the previous state or ZERO if absent
    ///
    /// For branches this is defined as follows:
    /// `hash([left.summary_hash, right.summary_hash])` if both present
    /// `x.summary_hash` if only one is present
    /// ZERO if both are absent
    pub summary_hash: HashOutTarget,
}

pub struct LeafTargets {
    /// The public inputs
    pub inputs: SubCircuitInputs,
}

impl SubCircuitInputs {
    pub fn default<F, const D: usize>(builder: &mut CircuitBuilder<F, D>) -> Self
    where
        F: RichField + Extendable<D>, {
        let summary_hash_present = builder.add_virtual_bool_target_safe();
        let summary_hash = builder.add_virtual_hash();
        builder.register_public_input(summary_hash_present.target);
        builder.register_public_inputs(&summary_hash.elements);
        Self {
            summary_hash_present,
            summary_hash,
        }
    }

    #[must_use]
    pub fn build_leaf<F, const D: usize>(self, builder: &mut CircuitBuilder<F, D>) -> LeafTargets
    where
        F: RichField + Extendable<D>, {
        // prove hashes align with presence
        for e in self.summary_hash.elements {
            let e = builder.is_nonzero(e);
            builder.connect(e.target, self.summary_hash_present.target);
        }

        LeafTargets { inputs: self }
    }
}

pub struct LeafSubCircuit {
    pub targets: LeafTargets,
    pub indices: PublicIndices,
}

impl LeafTargets {
    #[must_use]
    pub fn build(self, public_inputs: &[Target]) -> LeafSubCircuit {
        // Find the indicies
        let indices = PublicIndices {
            summary_hash_present: find_bool(public_inputs, self.inputs.summary_hash_present),
            summary_hash: find_hash(public_inputs, self.inputs.summary_hash),
        };
        LeafSubCircuit {
            targets: self,
            indices,
        }
    }
}

impl LeafSubCircuit {
    pub fn set_witness<F: RichField>(
        &self,
        inputs: &mut PartialWitness<F>,
        summary_hash: HashOut<F>,
    ) {
        self.set_witness_unsafe(inputs, summary_hash != HashOut::ZERO, summary_hash);
    }

    fn set_witness_unsafe<F: RichField>(
        &self,
        inputs: &mut PartialWitness<F>,
        summary_hash_present: bool,
        summary_hash: HashOut<F>,
    ) {
        inputs.set_bool_target(
            self.targets.inputs.summary_hash_present,
            summary_hash_present,
        );
        inputs.set_hash_target(self.targets.inputs.summary_hash, summary_hash);
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
    fn direction_from_node<const D: usize>(
        proof: &ProofWithPublicInputsTarget<D>,
        indices: &PublicIndices,
    ) -> SubCircuitInputs {
        let summary_hash_present = indices.get_summary_hash_present(&proof.public_inputs);
        let summary_hash_present = BoolTarget::new_unsafe(summary_hash_present);
        let summary_hash = HashOutTarget::from(indices.get_summary_hash(&proof.public_inputs));

        SubCircuitInputs {
            summary_hash_present,
            summary_hash,
        }
    }

    fn build_helper<F: RichField + Extendable<D>, const D: usize>(
        self,
        builder: &mut CircuitBuilder<F, D>,
        left: SubCircuitInputs,
        right: SubCircuitInputs,
    ) -> BranchTargets {
        let l_present = left.summary_hash_present;
        let l_hash = left.summary_hash.elements;
        let r_present = right.summary_hash_present;
        let r_hash = right.summary_hash.elements;

        // Construct the forwarding "hash".
        let summary_hash_calc = hash_or_forward(builder, l_present, l_hash, r_present, r_hash);
        let summary_hash_present_calc = builder.or(l_present, r_present);

        builder.connect(
            summary_hash_present_calc.target,
            self.summary_hash_present.target,
        );
        builder.connect_hashes(summary_hash_calc, self.summary_hash);

        BranchTargets {
            inputs: self,
            left,
            right,
        }
    }

    #[must_use]
    pub fn from_leaf<F: RichField + Extendable<D>, const D: usize>(
        self,
        builder: &mut CircuitBuilder<F, D>,
        leaf: &LeafSubCircuit,
        left_proof: &ProofWithPublicInputsTarget<D>,
        right_proof: &ProofWithPublicInputsTarget<D>,
    ) -> BranchTargets {
        let left = Self::direction_from_node(left_proof, &leaf.indices);
        let right = Self::direction_from_node(right_proof, &leaf.indices);
        self.build_helper(builder, left, right)
    }

    pub fn from_branch<F: RichField + Extendable<D>, const D: usize>(
        self,
        builder: &mut CircuitBuilder<F, D>,
        branch: &BranchSubCircuit,
        left_proof: &ProofWithPublicInputsTarget<D>,
        right_proof: &ProofWithPublicInputsTarget<D>,
    ) -> BranchTargets {
        let left = Self::direction_from_node(left_proof, &branch.indices);
        let right = Self::direction_from_node(right_proof, &branch.indices);
        self.build_helper(builder, left, right)
    }
}

pub struct BranchSubCircuit {
    pub targets: BranchTargets,
    pub indices: PublicIndices,
    /// The distance from the leaves (`0` being the lowest branch)
    /// Used for debugging
    pub dbg_height: usize,
}

impl BranchTargets {
    fn get_indices(&self, public_inputs: &[Target]) -> PublicIndices {
        PublicIndices {
            summary_hash_present: find_bool(public_inputs, self.inputs.summary_hash_present),
            summary_hash: find_hash(public_inputs, self.inputs.summary_hash),
        }
    }

    #[must_use]
    pub fn from_leaf(self, public_inputs: &[Target]) -> BranchSubCircuit {
        BranchSubCircuit {
            indices: self.get_indices(public_inputs),
            targets: self,
            dbg_height: 0,
        }
    }

    #[must_use]
    pub fn from_branch(
        self,
        branch: &BranchSubCircuit,
        public_inputs: &[Target],
    ) -> BranchSubCircuit {
        BranchSubCircuit {
            indices: self.get_indices(public_inputs),
            targets: self,
            dbg_height: branch.dbg_height + 1,
        }
    }
}

impl BranchSubCircuit {
    pub fn set_witness<F: RichField>(
        &self,
        inputs: &mut PartialWitness<F>,
        summary_hash: HashOut<F>,
    ) {
        self.set_witness_unsafe(inputs, summary_hash != HashOut::ZERO, summary_hash);
    }

    fn set_witness_unsafe<F: RichField>(
        &self,
        inputs: &mut PartialWitness<F>,
        summary_hash_present: bool,
        summary_hash: HashOut<F>,
    ) {
        inputs.set_bool_target(
            self.targets.inputs.summary_hash_present,
            summary_hash_present,
        );
        inputs.set_hash_target(self.targets.inputs.summary_hash, summary_hash);
    }
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use lazy_static::lazy_static;
    use plonky2::field::types::Field;
    use plonky2::plonk::circuit_data::{CircuitConfig, CircuitData};
    use plonky2::plonk::proof::ProofWithPublicInputs;

    use super::*;
    use crate::recproof::unbounded;
    use crate::test_utils::{fast_test_circuit_config, hash_branch, hash_str, C, D, F};

    pub struct DummyLeafCircuit {
        pub summarized: LeafSubCircuit,
        pub unbounded: unbounded::LeafSubCircuit,
        pub circuit: CircuitData<F, C, D>,
    }

    impl DummyLeafCircuit {
        #[must_use]
        pub fn new(circuit_config: &CircuitConfig) -> Self {
            let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());

            let summarized_inputs = SubCircuitInputs::default(&mut builder);
            let summarized_targets = summarized_inputs.build_leaf(&mut builder);

            let (circuit, unbounded) = unbounded::LeafSubCircuit::new(builder);
            let summarized = summarized_targets.build(&circuit.prover_only.public_inputs);

            Self {
                summarized,
                unbounded,
                circuit,
            }
        }

        pub fn prove(
            &self,
            summary_hash: HashOut<F>,
            branch: &DummyBranchCircuit,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.summarized.set_witness(&mut inputs, summary_hash);
            self.unbounded.set_witness(&mut inputs, &branch.circuit);
            self.circuit.prove(inputs)
        }

        fn prove_unsafe(
            &self,
            summary_hash_present: bool,
            summary_hash: HashOut<F>,
            branch: &DummyBranchCircuit,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.summarized
                .set_witness_unsafe(&mut inputs, summary_hash_present, summary_hash);
            self.unbounded.set_witness(&mut inputs, &branch.circuit);
            self.circuit.prove(inputs)
        }
    }

    pub struct DummyBranchCircuit {
        pub summarized: BranchSubCircuit,
        pub unbounded: unbounded::BranchSubCircuit,
        pub circuit: CircuitData<F, C, D>,
        pub targets: DummyBranchTargets,
    }

    pub struct DummyBranchTargets {
        pub left_is_leaf: BoolTarget,
        pub right_is_leaf: BoolTarget,
        pub left_proof: ProofWithPublicInputsTarget<D>,
        pub right_proof: ProofWithPublicInputsTarget<D>,
    }

    impl DummyBranchCircuit {
        #[must_use]
        pub fn new(circuit_config: &CircuitConfig, leaf: &DummyLeafCircuit) -> Self {
            let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());

            let circuit_data = &leaf.circuit;
            let common = &circuit_data.common;
            let left_is_leaf = builder.add_virtual_bool_target_safe();
            let right_is_leaf = builder.add_virtual_bool_target_safe();
            let left_proof = builder.add_virtual_proof_with_pis(common);
            let right_proof = builder.add_virtual_proof_with_pis(common);
            let summarized_inputs = SubCircuitInputs::default(&mut builder);

            let summarized_targets = summarized_inputs.from_leaf(
                &mut builder,
                &leaf.summarized,
                &left_proof,
                &right_proof,
            );

            let (circuit, unbounded) = unbounded::BranchSubCircuit::new(
                builder,
                &leaf.circuit,
                left_is_leaf,
                right_is_leaf,
                &left_proof,
                &right_proof,
            );
            let targets = DummyBranchTargets {
                left_is_leaf,
                right_is_leaf,
                left_proof,
                right_proof,
            };
            let summarized = summarized_targets.from_leaf(&circuit.prover_only.public_inputs);

            Self {
                summarized,
                unbounded,
                circuit,
                targets,
            }
        }

        pub fn prove(
            &self,
            left_is_leaf: bool,
            right_is_leaf: bool,
            left_proof: &ProofWithPublicInputs<F, C, D>,
            right_proof: &ProofWithPublicInputs<F, C, D>,
            summary_hash: HashOut<F>,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            inputs.set_bool_target(self.targets.left_is_leaf, left_is_leaf);
            inputs.set_bool_target(self.targets.right_is_leaf, right_is_leaf);
            inputs.set_proof_with_pis_target(&self.targets.left_proof, left_proof);
            inputs.set_proof_with_pis_target(&self.targets.right_proof, right_proof);
            self.summarized.set_witness(&mut inputs, summary_hash);
            self.circuit.prove(inputs)
        }

        fn prove_unsafe(
            &self,
            left_is_leaf: bool,
            right_is_leaf: bool,
            left_proof: &ProofWithPublicInputs<F, C, D>,
            right_proof: &ProofWithPublicInputs<F, C, D>,
            summary_hash_present: bool,
            summary_hash: HashOut<F>,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            inputs.set_bool_target(self.targets.left_is_leaf, left_is_leaf);
            inputs.set_bool_target(self.targets.right_is_leaf, right_is_leaf);
            inputs.set_proof_with_pis_target(&self.targets.left_proof, left_proof);
            inputs.set_proof_with_pis_target(&self.targets.right_proof, right_proof);
            self.summarized
                .set_witness_unsafe(&mut inputs, summary_hash_present, summary_hash);
            self.circuit.prove(inputs)
        }
    }

    const CONFIG: CircuitConfig = fast_test_circuit_config();

    lazy_static! {
        static ref LEAF: DummyLeafCircuit = DummyLeafCircuit::new(&CONFIG);
        static ref BRANCH: DummyBranchCircuit = DummyBranchCircuit::new(&CONFIG, &LEAF);
    }

    #[test]
    fn verify_leaf() -> Result<()> {
        let zero_hash = HashOut::from([F::ZERO; NUM_HASH_OUT_ELTS]);
        let non_zero_hash = hash_str("Non-Zero Hash");

        let proof = LEAF.prove(zero_hash, &BRANCH)?;
        LEAF.circuit.verify(proof)?;

        let proof = LEAF.prove(non_zero_hash, &BRANCH)?;
        LEAF.circuit.verify(proof)?;

        Ok(())
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_zero_leaf() {
        let zero_hash = HashOut::from([F::ZERO; NUM_HASH_OUT_ELTS]);

        let proof = LEAF.prove_unsafe(true, zero_hash, &BRANCH).unwrap();
        LEAF.circuit.verify(proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_non_zero_leaf() {
        let non_zero_hash = hash_str("Non-Zero Hash");

        let proof = LEAF.prove_unsafe(false, non_zero_hash, &BRANCH).unwrap();
        LEAF.circuit.verify(proof).unwrap();
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn verify_branch() -> Result<()> {
        let zero_hash = HashOut::from([F::ZERO; NUM_HASH_OUT_ELTS]);
        let non_zero_hash_1 = hash_str("Non-Zero Hash 1");
        let non_zero_hash_2 = hash_str("Non-Zero Hash 2");
        let both_hash = hash_branch(&non_zero_hash_1, &non_zero_hash_2);

        // Leaf proofs
        let zero_proof = LEAF.prove(zero_hash, &BRANCH)?;
        LEAF.circuit.verify(zero_proof.clone())?;

        let non_zero_proof_1 = LEAF.prove(non_zero_hash_1, &BRANCH)?;
        LEAF.circuit.verify(non_zero_proof_1.clone())?;

        let non_zero_proof_2 = LEAF.prove(non_zero_hash_2, &BRANCH)?;
        LEAF.circuit.verify(non_zero_proof_2.clone())?;

        // Branch proofs
        let empty_branch_proof = BRANCH.prove(true, true, &zero_proof, &zero_proof, zero_hash)?;
        BRANCH.circuit.verify(empty_branch_proof.clone())?;

        let left1_branch_proof =
            BRANCH.prove(true, true, &non_zero_proof_1, &zero_proof, non_zero_hash_1)?;
        BRANCH.circuit.verify(left1_branch_proof.clone())?;

        let left2_branch_proof =
            BRANCH.prove(true, true, &non_zero_proof_2, &zero_proof, non_zero_hash_2)?;
        BRANCH.circuit.verify(left2_branch_proof.clone())?;

        let right1_branch_proof =
            BRANCH.prove(true, true, &zero_proof, &non_zero_proof_1, non_zero_hash_1)?;
        BRANCH.circuit.verify(right1_branch_proof.clone())?;

        let right2_branch_proof =
            BRANCH.prove(true, true, &zero_proof, &non_zero_proof_2, non_zero_hash_2)?;
        BRANCH.circuit.verify(right2_branch_proof.clone())?;

        let both_branch_proof =
            BRANCH.prove(true, true, &non_zero_proof_1, &non_zero_proof_2, both_hash)?;
        BRANCH.circuit.verify(both_branch_proof.clone())?;

        // Double branch proofs
        let empty_branch_2_proof = BRANCH.prove(
            false,
            false,
            &empty_branch_proof,
            &empty_branch_proof,
            zero_hash,
        )?;
        BRANCH.circuit.verify(empty_branch_2_proof)?;

        let left_branch_2_proof = BRANCH.prove(
            false,
            false,
            &left1_branch_proof,
            &empty_branch_proof,
            non_zero_hash_1,
        )?;
        BRANCH.circuit.verify(left_branch_2_proof)?;

        let left_branch_2_proof = BRANCH.prove(
            false,
            false,
            &empty_branch_proof,
            &left1_branch_proof,
            non_zero_hash_1,
        )?;
        BRANCH.circuit.verify(left_branch_2_proof)?;

        let right_branch_2_proof = BRANCH.prove(
            false,
            false,
            &right2_branch_proof,
            &empty_branch_proof,
            non_zero_hash_2,
        )?;
        BRANCH.circuit.verify(right_branch_2_proof)?;

        let right_branch_2_proof = BRANCH.prove(
            false,
            false,
            &empty_branch_proof,
            &right2_branch_proof,
            non_zero_hash_2,
        )?;
        BRANCH.circuit.verify(right_branch_2_proof)?;

        let both_branch_2_proof = BRANCH.prove(
            false,
            false,
            &left1_branch_proof,
            &left2_branch_proof,
            both_hash,
        )?;
        BRANCH.circuit.verify(both_branch_2_proof)?;

        let both_branch_2_proof = BRANCH.prove(
            false,
            false,
            &left1_branch_proof,
            &right2_branch_proof,
            both_hash,
        )?;
        BRANCH.circuit.verify(both_branch_2_proof)?;

        let both_branch_2_proof = BRANCH.prove(
            false,
            false,
            &right1_branch_proof,
            &left2_branch_proof,
            both_hash,
        )?;
        BRANCH.circuit.verify(both_branch_2_proof)?;

        let both_branch_2_proof = BRANCH.prove(
            false,
            false,
            &right1_branch_proof,
            &right2_branch_proof,
            both_hash,
        )?;
        BRANCH.circuit.verify(both_branch_2_proof)?;

        let both_branch_2_proof = BRANCH.prove(
            false,
            false,
            &both_branch_proof,
            &empty_branch_proof,
            both_hash,
        )?;
        BRANCH.circuit.verify(both_branch_2_proof)?;

        let both_branch_2_proof = BRANCH.prove(
            false,
            false,
            &empty_branch_proof,
            &both_branch_proof,
            both_hash,
        )?;
        BRANCH.circuit.verify(both_branch_2_proof)?;

        Ok(())
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_proof_branch() {
        let zero_hash = HashOut::from([F::ZERO; NUM_HASH_OUT_ELTS]);

        let zero_proof = LEAF.prove(zero_hash, &BRANCH).unwrap();
        LEAF.circuit.verify(zero_proof.clone()).unwrap();

        let bad_proof = LEAF.prove_unsafe(true, zero_hash, &BRANCH).unwrap();

        let empty_branch_proof = BRANCH
            .prove(true, true, &zero_proof, &bad_proof, zero_hash)
            .unwrap();
        BRANCH.circuit.verify(empty_branch_proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_zero_branch() {
        let zero_hash = HashOut::from([F::ZERO; NUM_HASH_OUT_ELTS]);

        let zero_proof = LEAF.prove(zero_hash, &BRANCH).unwrap();
        LEAF.circuit.verify(zero_proof.clone()).unwrap();

        let branch_proof = BRANCH
            .prove_unsafe(true, true, &zero_proof, &zero_proof, true, zero_hash)
            .unwrap();
        BRANCH.circuit.verify(branch_proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_non_zero_branch() {
        let zero_hash = HashOut::from([F::ZERO; NUM_HASH_OUT_ELTS]);
        let non_zero_hash = hash_str("Non-Zero Hash");

        let zero_proof = LEAF.prove(zero_hash, &BRANCH).unwrap();
        LEAF.circuit.verify(zero_proof.clone()).unwrap();

        let non_zero_proof = LEAF.prove(non_zero_hash, &BRANCH).unwrap();
        LEAF.circuit.verify(non_zero_proof.clone()).unwrap();

        let branch_proof = BRANCH
            .prove_unsafe(
                true,
                true,
                &zero_proof,
                &non_zero_proof,
                false,
                non_zero_hash,
            )
            .unwrap();
        BRANCH.circuit.verify(branch_proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_wrong_hash_branch() {
        let non_zero_hash_1 = hash_str("Non-Zero Hash 1");
        let non_zero_hash_2 = hash_str("Non-Zero Hash 2");

        let non_zero_proof_1 = LEAF.prove(non_zero_hash_1, &BRANCH).unwrap();
        LEAF.circuit.verify(non_zero_proof_1.clone()).unwrap();

        let non_zero_proof_2 = LEAF.prove(non_zero_hash_2, &BRANCH).unwrap();
        LEAF.circuit.verify(non_zero_proof_2.clone()).unwrap();

        let branch_proof = BRANCH
            .prove(
                true,
                true,
                &non_zero_proof_1,
                &non_zero_proof_2,
                non_zero_hash_1,
            )
            .unwrap();
        BRANCH.circuit.verify(branch_proof).unwrap();
    }
}

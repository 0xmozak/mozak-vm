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
use plonky2::hash::poseidon2::Poseidon2Hash;
use plonky2::iop::target::BoolTarget;
use plonky2::iop::witness::{PartialWitness, WitnessWrite};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::CircuitData;
use plonky2::plonk::config::GenericConfig;
use plonky2::plonk::proof::ProofWithPublicInputsTarget;

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

pub struct LeafSubCircuit {
    pub targets: LeafTargets,
    pub indices: PublicIndices,
}

pub struct LeafTargets {
    pub summary_hash_present: BoolTarget,

    /// The hash of the previous state or ZERO if absent
    pub summary_hash: HashOutTarget,
}

impl LeafSubCircuit {
    #[must_use]
    pub fn new<F, C, const D: usize, B, R>(
        mut builder: CircuitBuilder<F, D>,
        build: B,
    ) -> (CircuitData<F, C, D>, (Self, R))
    where
        B: FnOnce(&LeafTargets, CircuitBuilder<F, D>) -> (CircuitData<F, C, D>, R),
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F>, {
        let summary_hash_present = builder.add_virtual_bool_target_safe();
        let summary_hash = builder.add_virtual_hash();

        // prove hashes align with presence
        for e in summary_hash.elements {
            let e = builder.is_nonzero(e);
            builder.connect(e.target, summary_hash_present.target);
        }

        builder.register_public_input(summary_hash_present.target);
        builder.register_public_inputs(&summary_hash.elements);

        let targets = LeafTargets {
            summary_hash_present,
            summary_hash,
        };
        let (circuit, r) = build(&targets, builder);
        let public_inputs = &circuit.prover_only.public_inputs;

        let indices = PublicIndices {
            summary_hash_present: public_inputs
                .iter()
                .position(|&pi| pi == targets.summary_hash_present.target)
                .expect("target not found"),
            summary_hash: targets.summary_hash.elements.map(|target| {
                public_inputs
                    .iter()
                    .position(|&pi| pi == target)
                    .expect("target not found")
            }),
        };
        let v = Self { targets, indices };

        (circuit, (v, r))
    }

    pub fn set_inputs<F: RichField>(
        &self,
        inputs: &mut PartialWitness<F>,
        summary_hash: HashOut<F>,
    ) {
        self.set_inputs_unsafe(inputs, summary_hash != HashOut::ZERO, summary_hash);
    }

    fn set_inputs_unsafe<F: RichField>(
        &self,
        inputs: &mut PartialWitness<F>,
        summary_hash_present: bool,
        summary_hash: HashOut<F>,
    ) {
        inputs.set_bool_target(self.targets.summary_hash_present, summary_hash_present);
        inputs.set_hash_target(self.targets.summary_hash, summary_hash);
    }
}

pub struct BranchSubCircuit {
    pub targets: BranchTargets,
    pub indices: PublicIndices,
    /// The distance from the leaves (`0` being the lowest branch)
    /// Used for debugging
    pub dbg_height: usize,
}

pub struct BranchTargets {
    /// The left direction
    pub left: BranchDirectionTargets,

    /// The right direction
    pub right: BranchDirectionTargets,

    pub summary_hash_present: BoolTarget,

    /// `hash([left.summary_hash, right.summary_hash])` if both present
    /// `x.summary_hash` if only one is present
    /// ZERO if both are absent
    pub summary_hash: HashOutTarget,
}

pub struct BranchDirectionTargets {
    pub summary_hash_present: BoolTarget,

    /// The hash of this direction proved by the associated proof or ZERO if
    /// absent
    pub summary_hash: HashOutTarget,
}

impl BranchSubCircuit {
    fn from_directions<F, C, const D: usize, B, R>(
        mut builder: CircuitBuilder<F, D>,
        left: BranchDirectionTargets,
        right: BranchDirectionTargets,
        dbg_height: usize,
        build: B,
    ) -> (CircuitData<F, C, D>, (Self, R))
    where
        B: FnOnce(&BranchTargets, CircuitBuilder<F, D>) -> (CircuitData<F, C, D>, R),
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F>, {
        let summary_hash_present =
            builder.or(left.summary_hash_present, right.summary_hash_present);
        let both_present = builder.and(left.summary_hash_present, right.summary_hash_present);
        let not_both_present = builder.not(both_present);

        // Construct the hash of [left, right]
        let hash_both = builder.hash_n_to_hash_no_pad::<Poseidon2Hash>(
            left.summary_hash
                .elements
                .into_iter()
                .chain(right.summary_hash.elements)
                .collect(),
        );
        // zero it out if we don't have both sides
        let hash_both = hash_both
            .elements
            .map(|e| builder.mul(e, both_present.target));

        // Construct the forwarding "hash".
        // Since absent sides will be zero, we can just sum.
        let hash_absent = [0, 1, 2, 3].map(|i| {
            builder.add(
                left.summary_hash.elements[i],
                right.summary_hash.elements[i],
            )
        });
        // zero it out if we DO have both sides
        let hash_absent = hash_absent.map(|e| builder.mul(e, not_both_present.target));

        let summary_hash = [0, 1, 2, 3].map(|i| builder.add(hash_both[i], hash_absent[i]));

        builder.register_public_input(summary_hash_present.target);
        builder.register_public_inputs(&summary_hash);

        let targets = BranchTargets {
            left,
            right,
            summary_hash_present,
            summary_hash: HashOutTarget::from(summary_hash),
        };
        let (circuit, r) = build(&targets, builder);
        let public_inputs = &circuit.prover_only.public_inputs;

        let indices = PublicIndices {
            summary_hash_present: public_inputs
                .iter()
                .position(|&pi| pi == targets.summary_hash_present.target)
                .expect("target not found"),
            summary_hash: targets.summary_hash.elements.map(|target| {
                public_inputs
                    .iter()
                    .position(|&pi| pi == target)
                    .expect("target not found")
            }),
        };
        let v = Self {
            targets,
            indices,
            dbg_height,
        };

        (circuit, (v, r))
    }

    fn direction_from_node<const D: usize>(
        proof: &ProofWithPublicInputsTarget<D>,
        indices: &PublicIndices,
    ) -> BranchDirectionTargets {
        let summary_hash_present = indices.get_summary_hash_present(&proof.public_inputs);
        let summary_hash_present = BoolTarget::new_unsafe(summary_hash_present);
        let summary_hash = HashOutTarget::from(indices.get_summary_hash(&proof.public_inputs));

        BranchDirectionTargets {
            summary_hash_present,
            summary_hash,
        }
    }

    pub fn from_leaf<F, C, const D: usize, B, R>(
        builder: CircuitBuilder<F, D>,
        leaf: &LeafSubCircuit,
        left_proof: &ProofWithPublicInputsTarget<D>,
        right_proof: &ProofWithPublicInputsTarget<D>,
        build: B,
    ) -> (CircuitData<F, C, D>, (Self, R))
    where
        B: FnOnce(&BranchTargets, CircuitBuilder<F, D>) -> (CircuitData<F, C, D>, R),
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F>, {
        let left_dir = Self::direction_from_node(left_proof, &leaf.indices);
        let right_dir = Self::direction_from_node(right_proof, &leaf.indices);
        let dbg_height = 0;
        Self::from_directions(builder, left_dir, right_dir, dbg_height, build)
    }

    pub fn from_branch<F, C, const D: usize, B, R>(
        builder: CircuitBuilder<F, D>,
        branch: &BranchSubCircuit,
        left_proof: &ProofWithPublicInputsTarget<D>,
        right_proof: &ProofWithPublicInputsTarget<D>,
        build: B,
    ) -> (CircuitData<F, C, D>, (Self, R))
    where
        B: FnOnce(&BranchTargets, CircuitBuilder<F, D>) -> (CircuitData<F, C, D>, R),
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F>, {
        let left_dir = Self::direction_from_node(left_proof, &branch.indices);
        let right_dir = Self::direction_from_node(right_proof, &branch.indices);
        let dbg_height = branch.dbg_height + 1;
        Self::from_directions(builder, left_dir, right_dir, dbg_height, build)
    }

    pub fn set_inputs<F: RichField>(
        &self,
        inputs: &mut PartialWitness<F>,
        summary_hash: HashOut<F>,
    ) {
        self.set_inputs_unsafe(inputs, summary_hash != HashOut::ZERO, summary_hash);
    }

    fn set_inputs_unsafe<F: RichField>(
        &self,
        inputs: &mut PartialWitness<F>,
        summary_hash_present: bool,
        summary_hash: HashOut<F>,
    ) {
        inputs.set_bool_target(self.targets.summary_hash_present, summary_hash_present);
        inputs.set_hash_target(self.targets.summary_hash, summary_hash);
    }
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use plonky2::field::types::Field;
    use plonky2::plonk::circuit_data::CircuitConfig;
    use plonky2::plonk::proof::ProofWithPublicInputs;

    use super::*;
    use crate::test_utils::{hash_branch, hash_str, C, D, F};

    pub struct DummyLeafCircuit {
        pub summarized: LeafSubCircuit,
        pub circuit: CircuitData<F, C, D>,
    }

    impl DummyLeafCircuit {
        #[must_use]
        pub fn new(circuit_config: &CircuitConfig) -> Self {
            let builder = CircuitBuilder::<F, D>::new(circuit_config.clone());
            let (circuit, (summarized, ())) =
                LeafSubCircuit::new(builder, |_targets, builder| (builder.build(), ()));

            Self {
                summarized,
                circuit,
            }
        }

        pub fn prove(&self, summary_hash: HashOut<F>) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.summarized.set_inputs(&mut inputs, summary_hash);
            self.circuit.prove(inputs)
        }

        fn prove_unsafe(
            &self,
            summary_hash_present: bool,
            summary_hash: HashOut<F>,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.summarized
                .set_inputs_unsafe(&mut inputs, summary_hash_present, summary_hash);
            self.circuit.prove(inputs)
        }
    }

    pub struct DummyBranchCircuit {
        pub summarized: BranchSubCircuit,
        pub circuit: CircuitData<F, C, D>,
        pub targets: DummyBranchTargets,
    }

    pub struct DummyBranchTargets {
        pub left_proof: ProofWithPublicInputsTarget<D>,
        pub right_proof: ProofWithPublicInputsTarget<D>,
    }

    impl DummyBranchCircuit {
        #[must_use]
        pub fn from_leaf(circuit_config: &CircuitConfig, leaf: &DummyLeafCircuit) -> Self {
            let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());

            let circuit_data = &leaf.circuit;
            let common = &circuit_data.common;
            let verifier = builder.constant_verifier_data(&circuit_data.verifier_only);
            let left_proof = builder.add_virtual_proof_with_pis(common);
            let right_proof = builder.add_virtual_proof_with_pis(common);
            builder.verify_proof::<C>(&left_proof, &verifier, common);
            builder.verify_proof::<C>(&right_proof, &verifier, common);

            let (circuit, (summarized, ())) = BranchSubCircuit::from_leaf(
                builder,
                &leaf.summarized,
                &left_proof,
                &right_proof,
                |_targets, builder| (builder.build(), ()),
            );

            let targets = DummyBranchTargets {
                left_proof,
                right_proof,
            };

            Self {
                summarized,
                circuit,
                targets,
            }
        }

        pub fn from_branch(circuit_config: &CircuitConfig, branch: &DummyBranchCircuit) -> Self {
            let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());

            let circuit_data = &branch.circuit;
            let common = &circuit_data.common;
            let verifier = builder.constant_verifier_data(&circuit_data.verifier_only);
            let left_proof = builder.add_virtual_proof_with_pis(common);
            let right_proof = builder.add_virtual_proof_with_pis(common);
            builder.verify_proof::<C>(&left_proof, &verifier, common);
            builder.verify_proof::<C>(&right_proof, &verifier, common);

            let (circuit, (summarized, ())) = BranchSubCircuit::from_branch(
                builder,
                &branch.summarized,
                &left_proof,
                &right_proof,
                |_targets, builder| (builder.build(), ()),
            );

            let targets = DummyBranchTargets {
                left_proof,
                right_proof,
            };

            Self {
                summarized,
                circuit,
                targets,
            }
        }

        pub fn prove(
            &self,
            left_proof: &ProofWithPublicInputs<F, C, D>,
            right_proof: &ProofWithPublicInputs<F, C, D>,
            summary_hash: HashOut<F>,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            inputs.set_proof_with_pis_target(&self.targets.left_proof, left_proof);
            inputs.set_proof_with_pis_target(&self.targets.right_proof, right_proof);
            self.summarized.set_inputs(&mut inputs, summary_hash);
            self.circuit.prove(inputs)
        }

        fn prove_unsafe(
            &self,
            left_proof: &ProofWithPublicInputs<F, C, D>,
            right_proof: &ProofWithPublicInputs<F, C, D>,
            summary_hash_present: bool,
            summary_hash: HashOut<F>,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            inputs.set_proof_with_pis_target(&self.targets.left_proof, left_proof);
            inputs.set_proof_with_pis_target(&self.targets.right_proof, right_proof);
            self.summarized
                .set_inputs_unsafe(&mut inputs, summary_hash_present, summary_hash);
            self.circuit.prove(inputs)
        }
    }

    #[test]
    fn verify_leaf() -> Result<()> {
        let circuit_config = CircuitConfig::standard_recursion_config();
        let circuit = DummyLeafCircuit::new(&circuit_config);

        let zero_hash = HashOut::from([F::ZERO; 4]);
        let non_zero_hash = hash_str("Non-Zero Hash");

        let proof = circuit.prove(zero_hash)?;
        circuit.circuit.verify(proof)?;

        let proof = circuit.prove(non_zero_hash)?;
        circuit.circuit.verify(proof)?;

        Ok(())
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_zero_leaf() {
        let circuit_config = CircuitConfig::standard_recursion_config();
        let circuit = DummyLeafCircuit::new(&circuit_config);

        let zero_hash = HashOut::from([F::ZERO; 4]);

        let proof = circuit.prove_unsafe(true, zero_hash).unwrap();
        circuit.circuit.verify(proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_non_zero_leaf() {
        let circuit_config = CircuitConfig::standard_recursion_config();
        let circuit = DummyLeafCircuit::new(&circuit_config);

        let non_zero_hash = hash_str("Non-Zero Hash");

        let proof = circuit.prove_unsafe(false, non_zero_hash).unwrap();
        circuit.circuit.verify(proof).unwrap();
    }

    #[test]
    fn verify_branch() -> Result<()> {
        let circuit_config = CircuitConfig::standard_recursion_config();
        let leaf_circuit = DummyLeafCircuit::new(&circuit_config);
        let branch_circuit_1 = DummyBranchCircuit::from_leaf(&circuit_config, &leaf_circuit);
        let branch_circuit_2 = DummyBranchCircuit::from_branch(&circuit_config, &branch_circuit_1);

        let zero_hash = HashOut::from([F::ZERO; 4]);
        let non_zero_hash_1 = hash_str("Non-Zero Hash 1");
        let non_zero_hash_2 = hash_str("Non-Zero Hash 2");
        let both_hash = hash_branch(&non_zero_hash_1, &non_zero_hash_2);

        // Leaf proofs
        let zero_proof = leaf_circuit.prove(zero_hash)?;
        leaf_circuit.circuit.verify(zero_proof.clone())?;

        let non_zero_proof_1 = leaf_circuit.prove(non_zero_hash_1)?;
        leaf_circuit.circuit.verify(non_zero_proof_1.clone())?;

        let non_zero_proof_2 = leaf_circuit.prove(non_zero_hash_2)?;
        leaf_circuit.circuit.verify(non_zero_proof_2.clone())?;

        // Branch proofs
        let empty_branch_proof = branch_circuit_1.prove(&zero_proof, &zero_proof, zero_hash)?;
        branch_circuit_1
            .circuit
            .verify(empty_branch_proof.clone())?;

        let left1_branch_proof =
            branch_circuit_1.prove(&non_zero_proof_1, &zero_proof, non_zero_hash_1)?;
        branch_circuit_1
            .circuit
            .verify(left1_branch_proof.clone())?;

        let left2_branch_proof =
            branch_circuit_1.prove(&non_zero_proof_2, &zero_proof, non_zero_hash_2)?;
        branch_circuit_1
            .circuit
            .verify(left2_branch_proof.clone())?;

        let right1_branch_proof =
            branch_circuit_1.prove(&zero_proof, &non_zero_proof_1, non_zero_hash_1)?;
        branch_circuit_1
            .circuit
            .verify(right1_branch_proof.clone())?;

        let right2_branch_proof =
            branch_circuit_1.prove(&zero_proof, &non_zero_proof_2, non_zero_hash_2)?;
        branch_circuit_1
            .circuit
            .verify(right2_branch_proof.clone())?;

        let both_branch_proof =
            branch_circuit_1.prove(&non_zero_proof_1, &non_zero_proof_2, both_hash)?;
        branch_circuit_1.circuit.verify(both_branch_proof.clone())?;

        // Double branch proofs
        let empty_branch_2_proof =
            branch_circuit_2.prove(&empty_branch_proof, &empty_branch_proof, zero_hash)?;
        branch_circuit_2.circuit.verify(empty_branch_2_proof)?;

        let left_branch_2_proof =
            branch_circuit_2.prove(&left1_branch_proof, &empty_branch_proof, non_zero_hash_1)?;
        branch_circuit_2.circuit.verify(left_branch_2_proof)?;

        let left_branch_2_proof =
            branch_circuit_2.prove(&empty_branch_proof, &left1_branch_proof, non_zero_hash_1)?;
        branch_circuit_2.circuit.verify(left_branch_2_proof)?;

        let right_branch_2_proof =
            branch_circuit_2.prove(&right2_branch_proof, &empty_branch_proof, non_zero_hash_2)?;
        branch_circuit_2.circuit.verify(right_branch_2_proof)?;

        let right_branch_2_proof =
            branch_circuit_2.prove(&empty_branch_proof, &right2_branch_proof, non_zero_hash_2)?;
        branch_circuit_2.circuit.verify(right_branch_2_proof)?;

        let both_branch_2_proof =
            branch_circuit_2.prove(&left1_branch_proof, &left2_branch_proof, both_hash)?;
        branch_circuit_2.circuit.verify(both_branch_2_proof)?;

        let both_branch_2_proof =
            branch_circuit_2.prove(&left1_branch_proof, &right2_branch_proof, both_hash)?;
        branch_circuit_2.circuit.verify(both_branch_2_proof)?;

        let both_branch_2_proof =
            branch_circuit_2.prove(&right1_branch_proof, &left2_branch_proof, both_hash)?;
        branch_circuit_2.circuit.verify(both_branch_2_proof)?;

        let both_branch_2_proof =
            branch_circuit_2.prove(&right1_branch_proof, &right2_branch_proof, both_hash)?;
        branch_circuit_2.circuit.verify(both_branch_2_proof)?;

        let both_branch_2_proof =
            branch_circuit_2.prove(&both_branch_proof, &empty_branch_proof, both_hash)?;
        branch_circuit_2.circuit.verify(both_branch_2_proof)?;

        let both_branch_2_proof =
            branch_circuit_2.prove(&empty_branch_proof, &both_branch_proof, both_hash)?;
        branch_circuit_2.circuit.verify(both_branch_2_proof)?;

        Ok(())
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_proof_branch() {
        let circuit_config = CircuitConfig::standard_recursion_config();
        let leaf_circuit = DummyLeafCircuit::new(&circuit_config);
        let branch_circuit_1 = DummyBranchCircuit::from_leaf(&circuit_config, &leaf_circuit);

        let zero_hash = HashOut::from([F::ZERO; 4]);

        let zero_proof = leaf_circuit.prove(zero_hash).unwrap();
        leaf_circuit.circuit.verify(zero_proof.clone()).unwrap();

        let bad_proof = leaf_circuit.prove_unsafe(true, zero_hash).unwrap();

        let empty_branch_proof = branch_circuit_1
            .prove(&zero_proof, &bad_proof, zero_hash)
            .unwrap();
        branch_circuit_1.circuit.verify(empty_branch_proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_zero_branch() {
        let circuit_config = CircuitConfig::standard_recursion_config();
        let leaf_circuit = DummyLeafCircuit::new(&circuit_config);
        let branch_circuit_1 = DummyBranchCircuit::from_leaf(&circuit_config, &leaf_circuit);

        let zero_hash = HashOut::from([F::ZERO; 4]);

        let zero_proof = leaf_circuit.prove(zero_hash).unwrap();
        leaf_circuit.circuit.verify(zero_proof.clone()).unwrap();

        let branch_proof = branch_circuit_1
            .prove_unsafe(&zero_proof, &zero_proof, true, zero_hash)
            .unwrap();
        branch_circuit_1.circuit.verify(branch_proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_non_zero_branch() {
        let circuit_config = CircuitConfig::standard_recursion_config();
        let leaf_circuit = DummyLeafCircuit::new(&circuit_config);
        let branch_circuit_1 = DummyBranchCircuit::from_leaf(&circuit_config, &leaf_circuit);

        let zero_hash = HashOut::from([F::ZERO; 4]);
        let non_zero_hash = hash_str("Non-Zero Hash");

        let zero_proof = leaf_circuit.prove(zero_hash).unwrap();
        leaf_circuit.circuit.verify(zero_proof.clone()).unwrap();

        let non_zero_proof = leaf_circuit.prove(non_zero_hash).unwrap();
        leaf_circuit.circuit.verify(non_zero_proof.clone()).unwrap();

        let branch_proof = branch_circuit_1
            .prove_unsafe(&zero_proof, &non_zero_proof, false, non_zero_hash)
            .unwrap();
        branch_circuit_1.circuit.verify(branch_proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_wrong_hash_branch() {
        let circuit_config = CircuitConfig::standard_recursion_config();
        let leaf_circuit = DummyLeafCircuit::new(&circuit_config);
        let branch_circuit_1 = DummyBranchCircuit::from_leaf(&circuit_config, &leaf_circuit);

        let non_zero_hash_1 = hash_str("Non-Zero Hash 1");
        let non_zero_hash_2 = hash_str("Non-Zero Hash 2");

        let non_zero_proof_1 = leaf_circuit.prove(non_zero_hash_1).unwrap();
        leaf_circuit
            .circuit
            .verify(non_zero_proof_1.clone())
            .unwrap();

        let non_zero_proof_2 = leaf_circuit.prove(non_zero_hash_2).unwrap();
        leaf_circuit
            .circuit
            .verify(non_zero_proof_2.clone())
            .unwrap();

        let branch_proof = branch_circuit_1
            .prove(&non_zero_proof_1, &non_zero_proof_2, non_zero_hash_1)
            .unwrap();
        branch_circuit_1.circuit.verify(branch_proof).unwrap();
    }
}

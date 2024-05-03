//! Subcircuits for recursively proving addresses of leaves of a merkle
//! tree are correct.

use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::target::{BoolTarget, Target};
use plonky2::iop::witness::{PartialWitness, WitnessWrite};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::proof::ProofWithPublicInputsTarget;

use crate::indices::{BoolTargetIndex, TargetIndex};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct PublicIndices {
    pub node_present: BoolTargetIndex,
    pub node_address: TargetIndex,
}

pub struct SubCircuitInputs {
    pub node_present: BoolTarget,

    /// The address of this node or `-1` if absent
    pub node_address: Target,
}

pub struct LeafTargets {
    pub node_present: BoolTarget,

    /// The address of this node or `-1` if absent
    pub node_address: Target,
}

impl SubCircuitInputs {
    pub fn default<F, const D: usize>(builder: &mut CircuitBuilder<F, D>) -> Self
    where
        F: RichField + Extendable<D>, {
        let node_present = builder.add_virtual_bool_target_safe();
        let node_address = builder.add_virtual_target();
        builder.register_public_input(node_present.target);
        builder.register_public_input(node_address);
        Self {
            node_present,
            node_address,
        }
    }

    #[must_use]
    pub fn build_leaf<F, const D: usize>(self, builder: &mut CircuitBuilder<F, D>) -> LeafTargets
    where
        F: RichField + Extendable<D>, {
        let Self {
            node_present,
            node_address,
        } = self;

        // if `node_address == -1`, set `check_neg_one` to `0`
        // Note all other values will result in non-zero
        let check_neg_one = builder.add_const(node_address, F::ONE);
        let node_present_calc = builder.is_nonzero(check_neg_one);
        builder.connect(node_present_calc.target, node_present.target);

        LeafTargets {
            node_present,
            node_address,
        }
    }
}

pub struct LeafSubCircuit {
    pub targets: LeafTargets,
    pub indices: PublicIndices,
}

impl LeafTargets {
    #[must_use]
    pub fn build(self, public_inputs: &[Target]) -> LeafSubCircuit {
        let indices = PublicIndices {
            node_present: BoolTargetIndex::new(public_inputs, self.node_present),
            node_address: TargetIndex::new(public_inputs, self.node_address),
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
        node_address: Option<u64>,
    ) {
        self.set_witness_unsafe(inputs, node_address.is_some(), node_address);
    }

    fn set_witness_unsafe<F: RichField>(
        &self,
        inputs: &mut PartialWitness<F>,
        node_present: bool,
        node_address: Option<u64>,
    ) {
        let node_address = node_address.map_or(F::ZERO.sub_one(), F::from_canonical_u64);
        inputs.set_bool_target(self.targets.node_present, node_present);
        inputs.set_target(self.targets.node_address, node_address);
    }
}

pub struct BranchTargets {
    /// The public inputs
    pub inputs: SubCircuitInputs,

    /// The left direction
    /// Should have an even address which is one less than `right`
    pub left: SubCircuitInputs,

    /// The right direction
    /// Should have an odd address which is one more than `left`
    pub right: SubCircuitInputs,
}

impl SubCircuitInputs {
    fn direction_from_node<const D: usize>(
        proof: &ProofWithPublicInputsTarget<D>,
        indices: &PublicIndices,
    ) -> SubCircuitInputs {
        let node_present = indices.node_present.get(&proof.public_inputs);
        let node_address = indices.node_address.get(&proof.public_inputs);

        SubCircuitInputs {
            node_present,
            node_address,
        }
    }

    pub fn build_branch<F: RichField + Extendable<D>, const D: usize>(
        self,
        builder: &mut CircuitBuilder<F, D>,
        indices: &PublicIndices,
        left_proof: &ProofWithPublicInputsTarget<D>,
        right_proof: &ProofWithPublicInputsTarget<D>,
    ) -> BranchTargets {
        let left = Self::direction_from_node(left_proof, indices);
        let right = Self::direction_from_node(right_proof, indices);

        let one = builder.one();
        let two = builder.two();

        let l_present = left.node_present;
        let l_address = left.node_address;
        let r_present = right.node_present;
        let r_address = right.node_address;

        let both_present = builder.and(l_present, r_present);
        let node_present_calc = builder.or(l_present, r_present);
        // Parent nodes are the bitwise common prefix
        // so we just need the circuit equivalent of truncated division by 2
        let l_parent = builder.div(l_address, two);
        let r_parent = builder.add_const(r_address, -F::ONE);
        let r_parent = builder.div(r_parent, two);

        let parents_match = builder.is_equal(l_parent, r_parent);
        let parents_match = builder.select(both_present, parents_match.target, one);
        builder.connect(parents_match, one);

        // Account for "not present" values by forwarding the existing value
        let parent = builder.select(l_present, l_parent, r_parent);
        let node_address_calc = builder.select(node_present_calc, parent, l_address);

        builder.connect(node_present_calc.target, self.node_present.target);
        builder.connect(node_address_calc, self.node_address);

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
    pub fn build(self, child: &PublicIndices, public_inputs: &[Target]) -> BranchSubCircuit {
        let indices = PublicIndices {
            node_present: BoolTargetIndex::new(public_inputs, self.inputs.node_present),
            node_address: TargetIndex::new(public_inputs, self.inputs.node_address),
        };
        debug_assert_eq!(indices, *child);

        BranchSubCircuit {
            indices,
            targets: self,
        }
    }
}

impl BranchSubCircuit {
    /// This call is actually totally unnecessary, as the parent will
    /// be calculated from the child proofs, but it can be used to verify
    /// the parent is what you think it is.
    pub fn set_witness<F: RichField>(
        &self,
        inputs: &mut PartialWitness<F>,
        node_address: Option<u64>,
    ) {
        self.set_witness_unsafe(inputs, node_address.is_some(), node_address);
    }

    fn set_witness_unsafe<F: RichField>(
        &self,
        inputs: &mut PartialWitness<F>,
        node_present: bool,
        node_address: Option<u64>,
    ) {
        let node_address = node_address.map_or(F::ZERO.sub_one(), F::from_canonical_u64);
        inputs.set_bool_target(self.targets.inputs.node_present, node_present);
        inputs.set_target(self.targets.inputs.node_address, node_address);
    }
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use iter_fixed::IntoIteratorFixed;
    use lazy_static::lazy_static;
    use plonky2::plonk::circuit_data::{CircuitConfig, CircuitData};
    use plonky2::plonk::proof::ProofWithPublicInputs;

    use super::*;
    use crate::subcircuits::bounded;
    use crate::test_utils::{C, CONFIG, D, F};

    pub struct DummyLeafCircuit {
        pub bounded: bounded::LeafSubCircuit,
        pub address: LeafSubCircuit,
        pub circuit: CircuitData<F, C, D>,
    }

    impl DummyLeafCircuit {
        #[must_use]
        pub fn new(circuit_config: &CircuitConfig) -> Self {
            let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());

            let bounded_inputs = bounded::SubCircuitInputs::default(&mut builder);
            let address_inputs = SubCircuitInputs::default(&mut builder);

            let bounded_targets = bounded_inputs.build_leaf(&mut builder);
            let address_targets = address_inputs.build_leaf(&mut builder);

            let circuit = builder.build();

            let public_inputs = &circuit.prover_only.public_inputs;
            let bounded = bounded_targets.build(public_inputs);
            let address = address_targets.build(public_inputs);

            Self {
                bounded,
                address,
                circuit,
            }
        }

        pub fn prove(&self, node_address: Option<u64>) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.bounded.set_witness(&mut inputs);
            self.address.set_witness(&mut inputs, node_address);
            self.circuit.prove(inputs)
        }

        fn prove_unsafe(
            &self,
            node_present: bool,
            node_address: Option<u64>,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.bounded.set_witness(&mut inputs);
            self.address
                .set_witness_unsafe(&mut inputs, node_present, node_address);
            self.circuit.prove(inputs)
        }
    }

    pub struct DummyBranchCircuit {
        pub bounded: bounded::BranchSubCircuit<D>,
        pub address: BranchSubCircuit,
        pub circuit: CircuitData<F, C, D>,
    }

    impl DummyBranchCircuit {
        #[must_use]
        pub fn new(
            circuit_config: &CircuitConfig,
            indices: &PublicIndices,
            child: &CircuitData<F, C, D>,
        ) -> Self {
            let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());

            let bounded_inputs = bounded::SubCircuitInputs::default(&mut builder);
            let address_inputs = SubCircuitInputs::default(&mut builder);

            let bounded_targets = bounded_inputs.build_branch(&mut builder, child);
            let address_targets = address_inputs.build_branch(
                &mut builder,
                indices,
                &bounded_targets.left_proof,
                &bounded_targets.right_proof,
            );

            let circuit = builder.build();

            let public_inputs = &circuit.prover_only.public_inputs;
            let bounded = bounded_targets.build(public_inputs);
            let address = address_targets.build(indices, public_inputs);

            Self {
                bounded,
                address,
                circuit,
            }
        }

        #[must_use]
        pub fn from_leaf(circuit_config: &CircuitConfig, leaf: &DummyLeafCircuit) -> Self {
            Self::new(circuit_config, &leaf.address.indices, &leaf.circuit)
        }

        #[must_use]
        pub fn from_branch(circuit_config: &CircuitConfig, branch: &Self) -> Self {
            Self::new(circuit_config, &branch.address.indices, &branch.circuit)
        }

        pub fn prove(
            &self,
            node_address: Option<u64>,
            left_proof: &ProofWithPublicInputs<F, C, D>,
            right_proof: &ProofWithPublicInputs<F, C, D>,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.bounded
                .set_witness(&mut inputs, left_proof, right_proof);
            self.address.set_witness(&mut inputs, node_address);
            self.circuit.prove(inputs)
        }

        fn prove_unsafe(
            &self,
            node_present: bool,
            node_address: Option<u64>,
            left_proof: &ProofWithPublicInputs<F, C, D>,
            right_proof: &ProofWithPublicInputs<F, C, D>,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.bounded
                .set_witness(&mut inputs, left_proof, right_proof);
            self.address
                .set_witness_unsafe(&mut inputs, node_present, node_address);
            self.circuit.prove(inputs)
        }
    }

    lazy_static! {
        static ref LEAF: DummyLeafCircuit = DummyLeafCircuit::new(&CONFIG);
        static ref BRANCH_1: DummyBranchCircuit = DummyBranchCircuit::from_leaf(&CONFIG, &LEAF);
        static ref BRANCH_2: DummyBranchCircuit =
            DummyBranchCircuit::from_branch(&CONFIG, &BRANCH_1);
    }

    #[test]
    fn verify_leaf() -> Result<()> {
        let proof = LEAF.prove(None)?;
        LEAF.circuit.verify(proof)?;

        for i in 0..4 {
            let proof = LEAF.prove(Some(i))?;
            LEAF.circuit.verify(proof)?;
        }

        Ok(())
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_zero_leaf() {
        let proof = LEAF.prove_unsafe(true, None).unwrap();
        LEAF.circuit.verify(proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_non_zero_leaf() {
        let proof = LEAF.prove_unsafe(false, Some(5)).unwrap();
        LEAF.circuit.verify(proof).unwrap();
    }

    #[test]
    fn verify_branch() -> Result<()> {
        // Leaf proofs
        let zero_proof = LEAF.prove(None)?;
        LEAF.circuit.verify(zero_proof.clone())?;

        let leaf_proofs: [_; 4] = [0u8; 4]
            .into_iter_fixed()
            .enumerate()
            .map(|(i, _)| {
                let non_zero_proof = LEAF.prove(Some(i as u64)).unwrap();
                LEAF.circuit.verify(non_zero_proof.clone()).unwrap();
                non_zero_proof
            })
            .collect();

        // Branch proofs
        let empty_branch_proof = BRANCH_1.prove(None, &zero_proof, &zero_proof)?;
        BRANCH_1.circuit.verify(empty_branch_proof.clone())?;

        let left_branch_proofs: [_; 2] = [0u8; 2]
            .into_iter_fixed()
            .enumerate()
            .map(|(i, _)| {
                let non_zero_proof = BRANCH_1
                    .prove(Some(i as u64), &leaf_proofs[i * 2], &zero_proof)
                    .unwrap();
                BRANCH_1.circuit.verify(non_zero_proof.clone()).unwrap();
                non_zero_proof
            })
            .collect();

        let right_branch_proofs: [_; 2] = [0u8; 2]
            .into_iter_fixed()
            .enumerate()
            .map(|(i, _)| {
                let non_zero_proof = BRANCH_1
                    .prove(Some(i as u64), &zero_proof, &leaf_proofs[i * 2 + 1])
                    .unwrap();
                BRANCH_1.circuit.verify(non_zero_proof.clone()).unwrap();
                non_zero_proof
            })
            .collect();

        let full_branch_proofs: [_; 2] = [0u8; 2]
            .into_iter_fixed()
            .enumerate()
            .map(|(i, _)| {
                let non_zero_proof = BRANCH_1
                    .prove(Some(i as u64), &leaf_proofs[i * 2], &leaf_proofs[i * 2 + 1])
                    .unwrap();
                BRANCH_1.circuit.verify(non_zero_proof.clone()).unwrap();
                non_zero_proof
            })
            .collect();

        // Double branch proofs
        let empty_branch_2_proof =
            BRANCH_2.prove(None, &empty_branch_proof, &empty_branch_proof)?;
        BRANCH_2.circuit.verify(empty_branch_2_proof)?;

        let branches = [left_branch_proofs, right_branch_proofs, full_branch_proofs];
        for b in &branches {
            let non_zero_proof = BRANCH_2.prove(Some(0), &b[0], &empty_branch_proof)?;
            BRANCH_2.circuit.verify(non_zero_proof)?;

            let non_zero_proof = BRANCH_2.prove(Some(0), &empty_branch_proof, &b[1])?;
            BRANCH_2.circuit.verify(non_zero_proof)?;

            for b2 in &branches {
                let non_zero_proof = BRANCH_2.prove(Some(0), &b[0], &b2[1])?;
                BRANCH_2.circuit.verify(non_zero_proof)?;
            }
        }

        Ok(())
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_proof_branch() {
        let zero_proof = LEAF.prove(None).unwrap();
        LEAF.circuit.verify(zero_proof.clone()).unwrap();

        let bad_proof = LEAF.prove_unsafe(true, None).unwrap();

        let empty_branch_proof = BRANCH_1.prove(None, &zero_proof, &bad_proof).unwrap();
        BRANCH_1.circuit.verify(empty_branch_proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_zero_branch() {
        let zero_proof = LEAF.prove(None).unwrap();
        LEAF.circuit.verify(zero_proof.clone()).unwrap();

        let branch_proof = BRANCH_1
            .prove_unsafe(true, None, &zero_proof, &zero_proof)
            .unwrap();
        BRANCH_1.circuit.verify(branch_proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_non_zero_branch() {
        let zero_proof = LEAF.prove(None).unwrap();
        LEAF.circuit.verify(zero_proof.clone()).unwrap();

        let non_zero_proof = LEAF.prove(Some(5)).unwrap();
        LEAF.circuit.verify(non_zero_proof.clone()).unwrap();

        let branch_proof = BRANCH_1
            .prove_unsafe(false, Some(2), &zero_proof, &non_zero_proof)
            .unwrap();
        BRANCH_1.circuit.verify(branch_proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_wrong_parent_branch() {
        let non_zero_proof_1 = LEAF.prove(Some(4)).unwrap();
        LEAF.circuit.verify(non_zero_proof_1.clone()).unwrap();

        let non_zero_proof_2 = LEAF.prove(Some(5)).unwrap();
        LEAF.circuit.verify(non_zero_proof_2.clone()).unwrap();

        let branch_proof = BRANCH_1
            .prove(Some(3), &non_zero_proof_1, &non_zero_proof_2)
            .unwrap();
        BRANCH_1.circuit.verify(branch_proof).unwrap();
    }
}

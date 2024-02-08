//! Subcircuits for recursively proving addresses of select leaves of a merkle
//! tree are correct.
//!
//! These subcircuits are pseudo-recursive, building on top of each other to
//! create the next level up of the merkle tree. "Pseudo-" here means the height
//! must be fixed ahead of time and not depend on the content.
//!
//! These subcircuits are useful to prove knowledge of a selected subset of
//! nodes.
use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::target::{BoolTarget, Target};
use plonky2::iop::witness::{PartialWitness, WitnessWrite};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::proof::ProofWithPublicInputsTarget;

#[derive(Copy, Clone)]
pub struct PublicIndices {
    pub node_present: usize,
    pub node_address: usize,
}

impl PublicIndices {
    pub fn get_node_present<T: Copy>(&self, public_inputs: &[T]) -> T {
        public_inputs[self.node_present]
    }

    pub fn get_node_address<T: Copy>(&self, public_inputs: &[T]) -> T {
        public_inputs[self.node_address]
    }

    pub fn set_node_present<T>(&self, public_inputs: &mut [T], v: T) {
        public_inputs[self.node_present] = v;
    }

    pub fn set_node_address<T>(&self, public_inputs: &mut [T], v: T) {
        public_inputs[self.node_address] = v;
    }
}
pub struct LeafInputs {
    pub node_present: BoolTarget,

    /// The address of this node or `-1` if absent
    pub node_address: Target,
}

pub struct LeafTargets {
    pub node_present: BoolTarget,

    /// The address of this node or `-1` if absent
    pub node_address: Target,
}

impl LeafInputs {
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
    pub fn build<F, const D: usize>(self, builder: &mut CircuitBuilder<F, D>) -> LeafTargets
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
            node_present: public_inputs
                .iter()
                .position(|&pi| pi == self.node_present.target)
                .expect("target not found"),
            node_address: public_inputs
                .iter()
                .position(|&pi| pi == self.node_address)
                .expect("target not found"),
        };
        LeafSubCircuit {
            targets: self,
            indices,
        }
    }
}

impl LeafSubCircuit {
    pub fn set_inputs<F: RichField>(
        &self,
        inputs: &mut PartialWitness<F>,
        node_address: Option<u64>,
    ) {
        self.set_inputs_unsafe(inputs, node_address.is_some(), node_address);
    }

    fn set_inputs_unsafe<F: RichField>(
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

pub struct BranchInputs {
    pub node_present: BoolTarget,

    /// The address of this node or `-1` if absent
    pub node_address: Target,
}

pub struct BranchTargets {
    /// The left direction
    pub left: BranchDirectionTargets,

    /// The right direction
    pub right: BranchDirectionTargets,

    pub node_present: BoolTarget,

    /// The address of this node or `-1` if absent
    pub node_address: Target,
}

pub struct BranchDirectionTargets {
    pub node_present: BoolTarget,

    /// The address of this node or `-1` if absent
    pub node_address: Target,
}

impl BranchInputs {
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

    fn direction_from_node<const D: usize>(
        proof: &ProofWithPublicInputsTarget<D>,
        indices: &PublicIndices,
    ) -> BranchDirectionTargets {
        let node_present = indices.get_node_present(&proof.public_inputs);
        let node_present = BoolTarget::new_unsafe(node_present);
        let node_address = indices.get_node_address(&proof.public_inputs);

        BranchDirectionTargets {
            node_present,
            node_address,
        }
    }

    fn build_helper<F: RichField + Extendable<D>, const D: usize>(
        self,
        builder: &mut CircuitBuilder<F, D>,
        left: BranchDirectionTargets,
        right: BranchDirectionTargets,
    ) -> BranchTargets {
        let Self {
            node_present,
            node_address,
        } = self;
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

        builder.connect(node_present_calc.target, node_present.target);
        builder.connect(node_address_calc, node_address);

        BranchTargets {
            left,
            right,
            node_present,
            node_address,
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
            node_present: public_inputs
                .iter()
                .position(|&pi| pi == self.node_present.target)
                .expect("target not found"),
            node_address: public_inputs
                .iter()
                .position(|&pi| pi == self.node_address)
                .expect("target not found"),
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
    /// This call is actually totally unnecessary, as the parent will
    /// be calculated from the child proofs, but it can be used to verify
    /// the parent is what you think it is.
    pub fn set_inputs<F: RichField>(
        &self,
        inputs: &mut PartialWitness<F>,
        node_address: Option<u64>,
    ) {
        self.set_inputs_unsafe(inputs, node_address.is_some(), node_address);
    }

    fn set_inputs_unsafe<F: RichField>(
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

#[cfg(test)]
mod test {
    use anyhow::Result;
    use iter_fixed::IntoIteratorFixed;
    use plonky2::plonk::circuit_data::{CircuitConfig, CircuitData};
    use plonky2::plonk::proof::ProofWithPublicInputs;

    use super::*;
    use crate::test_utils::{C, D, F};

    pub struct DummyLeafCircuit {
        pub address: LeafSubCircuit,
        pub circuit: CircuitData<F, C, D>,
    }

    impl DummyLeafCircuit {
        #[must_use]
        pub fn new(circuit_config: &CircuitConfig) -> Self {
            let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());

            let address_inputs = LeafInputs::default(&mut builder);
            let address_targets = address_inputs.build(&mut builder);
            let circuit = builder.build();
            let address = address_targets.build(&circuit.prover_only.public_inputs);

            Self { address, circuit }
        }

        pub fn prove(&self, node_address: Option<u64>) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.address.set_inputs(&mut inputs, node_address);
            self.circuit.prove(inputs)
        }

        fn prove_unsafe(
            &self,
            node_present: bool,
            node_address: Option<u64>,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.address
                .set_inputs_unsafe(&mut inputs, node_present, node_address);
            self.circuit.prove(inputs)
        }
    }

    pub struct DummyBranchCircuit {
        pub address: BranchSubCircuit,
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
            let address_inputs = BranchInputs::default(&mut builder);

            builder.verify_proof::<C>(&left_proof, &verifier, common);
            builder.verify_proof::<C>(&right_proof, &verifier, common);
            let address_targets =
                address_inputs.from_leaf(&mut builder, &leaf.address, &left_proof, &right_proof);
            let targets = DummyBranchTargets {
                left_proof,
                right_proof,
            };

            let circuit = builder.build();
            let address = address_targets.from_leaf(&circuit.prover_only.public_inputs);

            Self {
                address,
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
            let address_inputs = BranchInputs::default(&mut builder);

            builder.verify_proof::<C>(&left_proof, &verifier, common);
            builder.verify_proof::<C>(&right_proof, &verifier, common);
            let address_targets = address_inputs.from_branch(
                &mut builder,
                &branch.address,
                &left_proof,
                &right_proof,
            );
            let targets = DummyBranchTargets {
                left_proof,
                right_proof,
            };

            let circuit = builder.build();
            let address =
                address_targets.from_branch(&branch.address, &circuit.prover_only.public_inputs);

            Self {
                address,
                circuit,
                targets,
            }
        }

        pub fn prove(
            &self,
            left_proof: &ProofWithPublicInputs<F, C, D>,
            right_proof: &ProofWithPublicInputs<F, C, D>,
            node_address: Option<u64>,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            inputs.set_proof_with_pis_target(&self.targets.left_proof, left_proof);
            inputs.set_proof_with_pis_target(&self.targets.right_proof, right_proof);
            self.address.set_inputs(&mut inputs, node_address);
            self.circuit.prove(inputs)
        }

        fn prove_unsafe(
            &self,
            left_proof: &ProofWithPublicInputs<F, C, D>,
            right_proof: &ProofWithPublicInputs<F, C, D>,
            node_present: bool,
            node_address: Option<u64>,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            inputs.set_proof_with_pis_target(&self.targets.left_proof, left_proof);
            inputs.set_proof_with_pis_target(&self.targets.right_proof, right_proof);
            self.address
                .set_inputs_unsafe(&mut inputs, node_present, node_address);
            self.circuit.prove(inputs)
        }
    }

    #[test]
    fn verify_leaf() -> Result<()> {
        let circuit_config = CircuitConfig::standard_recursion_config();
        let circuit = DummyLeafCircuit::new(&circuit_config);

        let proof = circuit.prove(None)?;
        circuit.circuit.verify(proof)?;

        for i in 0..4 {
            let proof = circuit.prove(Some(i))?;
            circuit.circuit.verify(proof)?;
        }

        Ok(())
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_zero_leaf() {
        let circuit_config = CircuitConfig::standard_recursion_config();
        let circuit = DummyLeafCircuit::new(&circuit_config);

        let proof = circuit.prove_unsafe(true, None).unwrap();
        circuit.circuit.verify(proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_non_zero_leaf_0() {
        let circuit_config = CircuitConfig::standard_recursion_config();
        let circuit = DummyLeafCircuit::new(&circuit_config);

        let proof = circuit.prove_unsafe(false, Some(0)).unwrap();
        circuit.circuit.verify(proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_non_zero_leaf_5() {
        let circuit_config = CircuitConfig::standard_recursion_config();
        let circuit = DummyLeafCircuit::new(&circuit_config);

        let proof = circuit.prove_unsafe(false, Some(5)).unwrap();
        circuit.circuit.verify(proof).unwrap();
    }

    #[test]
    fn verify_branch() -> Result<()> {
        let circuit_config = CircuitConfig::standard_recursion_config();
        let leaf_circuit = DummyLeafCircuit::new(&circuit_config);
        let branch_circuit_1 = DummyBranchCircuit::from_leaf(&circuit_config, &leaf_circuit);
        let branch_circuit_2 = DummyBranchCircuit::from_branch(&circuit_config, &branch_circuit_1);

        // Leaf proofs
        let zero_proof = leaf_circuit.prove(None)?;
        leaf_circuit.circuit.verify(zero_proof.clone())?;

        let leaf_proofs: [_; 4] = [0u8; 4]
            .into_iter_fixed()
            .enumerate()
            .map(|(i, _)| {
                let non_zero_proof = leaf_circuit.prove(Some(i as u64)).unwrap();
                leaf_circuit.circuit.verify(non_zero_proof.clone()).unwrap();
                non_zero_proof
            })
            .collect();

        // Branch proofs
        let empty_branch_proof = branch_circuit_1.prove(&zero_proof, &zero_proof, None)?;
        branch_circuit_1
            .circuit
            .verify(empty_branch_proof.clone())?;

        let left_branch_proofs: [_; 2] = [0u8; 2]
            .into_iter_fixed()
            .enumerate()
            .map(|(i, _)| {
                let non_zero_proof = branch_circuit_1
                    .prove(&leaf_proofs[i * 2], &zero_proof, Some(i as u64))
                    .unwrap();
                branch_circuit_1
                    .circuit
                    .verify(non_zero_proof.clone())
                    .unwrap();
                non_zero_proof
            })
            .collect();

        let right_branch_proofs: [_; 2] = [0u8; 2]
            .into_iter_fixed()
            .enumerate()
            .map(|(i, _)| {
                let non_zero_proof = branch_circuit_1
                    .prove(&zero_proof, &leaf_proofs[i * 2 + 1], Some(i as u64))
                    .unwrap();
                branch_circuit_1
                    .circuit
                    .verify(non_zero_proof.clone())
                    .unwrap();
                non_zero_proof
            })
            .collect();

        let full_branch_proofs: [_; 2] = [0u8; 2]
            .into_iter_fixed()
            .enumerate()
            .map(|(i, _)| {
                let non_zero_proof = branch_circuit_1
                    .prove(&leaf_proofs[i * 2], &leaf_proofs[i * 2 + 1], Some(i as u64))
                    .unwrap();
                branch_circuit_1
                    .circuit
                    .verify(non_zero_proof.clone())
                    .unwrap();
                non_zero_proof
            })
            .collect();

        // Double branch proofs
        let empty_branch_2_proof =
            branch_circuit_2.prove(&empty_branch_proof, &empty_branch_proof, None)?;
        branch_circuit_2.circuit.verify(empty_branch_2_proof)?;

        let branches = [left_branch_proofs, right_branch_proofs, full_branch_proofs];
        for b in &branches {
            let non_zero_proof = branch_circuit_2.prove(&b[0], &empty_branch_proof, Some(0))?;
            branch_circuit_2.circuit.verify(non_zero_proof)?;

            let non_zero_proof = branch_circuit_2.prove(&empty_branch_proof, &b[1], Some(0))?;
            branch_circuit_2.circuit.verify(non_zero_proof)?;

            for b2 in &branches {
                let non_zero_proof = branch_circuit_2.prove(&b[0], &b2[1], Some(0))?;
                branch_circuit_2.circuit.verify(non_zero_proof)?;
            }
        }

        Ok(())
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_proof_branch() {
        let circuit_config = CircuitConfig::standard_recursion_config();
        let leaf_circuit = DummyLeafCircuit::new(&circuit_config);
        let branch_circuit_1 = DummyBranchCircuit::from_leaf(&circuit_config, &leaf_circuit);

        let zero_proof = leaf_circuit.prove(None).unwrap();
        leaf_circuit.circuit.verify(zero_proof.clone()).unwrap();

        let bad_proof = leaf_circuit.prove_unsafe(true, None).unwrap();

        let empty_branch_proof = branch_circuit_1
            .prove(&zero_proof, &bad_proof, None)
            .unwrap();
        branch_circuit_1.circuit.verify(empty_branch_proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_zero_branch() {
        let circuit_config = CircuitConfig::standard_recursion_config();
        let leaf_circuit = DummyLeafCircuit::new(&circuit_config);
        let branch_circuit_1 = DummyBranchCircuit::from_leaf(&circuit_config, &leaf_circuit);

        let zero_proof = leaf_circuit.prove(None).unwrap();
        leaf_circuit.circuit.verify(zero_proof.clone()).unwrap();

        let branch_proof = branch_circuit_1
            .prove_unsafe(&zero_proof, &zero_proof, true, None)
            .unwrap();
        branch_circuit_1.circuit.verify(branch_proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_non_zero_branch() {
        let circuit_config = CircuitConfig::standard_recursion_config();
        let leaf_circuit = DummyLeafCircuit::new(&circuit_config);
        let branch_circuit_1 = DummyBranchCircuit::from_leaf(&circuit_config, &leaf_circuit);

        let zero_proof = leaf_circuit.prove(None).unwrap();
        leaf_circuit.circuit.verify(zero_proof.clone()).unwrap();

        let non_zero_proof = leaf_circuit.prove(Some(5)).unwrap();
        leaf_circuit.circuit.verify(non_zero_proof.clone()).unwrap();

        let branch_proof = branch_circuit_1
            .prove_unsafe(&zero_proof, &non_zero_proof, false, Some(2))
            .unwrap();
        branch_circuit_1.circuit.verify(branch_proof).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_wrong_parent_branch() {
        let circuit_config = CircuitConfig::standard_recursion_config();
        let leaf_circuit = DummyLeafCircuit::new(&circuit_config);
        let branch_circuit_1 = DummyBranchCircuit::from_leaf(&circuit_config, &leaf_circuit);

        let non_zero_proof_1 = leaf_circuit.prove(Some(4)).unwrap();
        leaf_circuit
            .circuit
            .verify(non_zero_proof_1.clone())
            .unwrap();

        let non_zero_proof_2 = leaf_circuit.prove(Some(5)).unwrap();
        leaf_circuit
            .circuit
            .verify(non_zero_proof_2.clone())
            .unwrap();

        let branch_proof = branch_circuit_1
            .prove(&non_zero_proof_1, &non_zero_proof_2, Some(3))
            .unwrap();
        branch_circuit_1.circuit.verify(branch_proof).unwrap();
    }
}

use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::{HashOut, HashOutTarget, RichField, NUM_HASH_OUT_ELTS};
use plonky2::hash::poseidon2::Poseidon2Hash;
use plonky2::iop::generator::{GeneratedValues, SimpleGenerator};
use plonky2::iop::target::{BoolTarget, Target};
use plonky2::iop::witness::{PartialWitness, PartitionWitness, Witness, WitnessWrite};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::{CircuitData, CommonCircuitData};
use plonky2::plonk::config::GenericConfig;
use plonky2::plonk::proof::ProofWithPublicInputsTarget;
use plonky2::util::serialization::{Buffer, IoResult, Read, Write};

use super::SubCircuit;

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
    pub fn new<F, C, const D: usize, T, B, R>(
        mut builder: CircuitBuilder<F, D>,
        t: T,
        build: B,
    ) -> (CircuitData<F, C, D>, (Self, R))
    where
        B: FnOnce(T, &LeafTargets, CircuitBuilder<F, D>) -> (CircuitData<F, C, D>, R),
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F>, {
        let summary_hash_present = builder.add_virtual_bool_target_safe();
        let summary_hash = builder.add_virtual_hash();

        // prove absent hashes are zero
        // `let hash_or_zero = if present { summary_hash } else { zero }`
        // let hash_or_zero = summary_hash.elements.map(|e|
        //     builder.mul(e, summary_hash_present.target)
        // );
        // // `assert_eq!(summary_hash, hash_or_zero)`
        // builder.connect_hashes(summary_hash, HashOutTarget::from(hash_or_zero));

        // prove hashes align with presence
        for e in summary_hash.elements {
            let e = is_nonzero(&mut builder, e);
            builder.connect(e.target, summary_hash_present.target);
        }

        builder.register_public_input(summary_hash_present.target);
        builder.register_public_inputs(&summary_hash.elements);

        let targets = LeafTargets {
            summary_hash_present,
            summary_hash,
        };
        let (circuit, r) = build(t, &targets, builder);
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

impl SubCircuit<PublicIndices> for LeafSubCircuit {
    fn pis(&self) -> usize { 5 }

    fn get_indices(&self) -> PublicIndices { self.indices }
}

pub struct BranchSubCircuit<'a, const D: usize> {
    pub targets: BranchTargets<D>,
    pub indices: PublicIndices,
    /// The distance from the leaves (`0`` being the lowest branch)
    /// Used for debugging
    pub height: usize,
    pub inner_circuit: &'a dyn SubCircuit<PublicIndices>,
}

pub struct BranchTargets<const D: usize> {
    /// The left dir
    pub left_dir: BranchDirTargets<D>,

    /// The right dir
    pub right_dir: BranchDirTargets<D>,

    pub summary_hash_present: BoolTarget,

    /// `hash([left.summary_hash, right.summary_hash])` if both present
    /// `x.summary_hash` if only one is present
    /// ZERO if both are absent
    pub summary_hash: HashOutTarget,
}

pub struct BranchDirTargets<const D: usize> {
    pub summary_hash_present: BoolTarget,

    /// The hash of this dir proved by `proof` or ZERO if absent
    pub summary_hash: HashOutTarget,
}

impl<'a, const D: usize> BranchSubCircuit<'a, D> {
    fn from_dirs<F, C, B, R>(
        inner_circuit: &'a dyn SubCircuit<PublicIndices>,
        mut builder: CircuitBuilder<F, D>,
        left_dir: BranchDirTargets<D>,
        right_dir: BranchDirTargets<D>,
        height: usize,
        build: B,
    ) -> (CircuitData<F, C, D>, (Self, R))
    where
        B: FnOnce(CircuitBuilder<F, D>) -> (CircuitData<F, C, D>, R),
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F>, {
        let summary_hash_present = builder.or(
            left_dir.summary_hash_present,
            right_dir.summary_hash_present,
        );
        let both_present = builder.and(
            left_dir.summary_hash_present,
            right_dir.summary_hash_present,
        );
        let not_both_present = builder.not(both_present);

        // Construct the hash of [left, right]
        let hash_both = builder.hash_n_to_hash_no_pad::<Poseidon2Hash>(
            left_dir
                .summary_hash
                .elements
                .into_iter()
                .chain(right_dir.summary_hash.elements)
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
                left_dir.summary_hash.elements[i],
                right_dir.summary_hash.elements[i],
            )
        });
        // zero it out if we DO have both sides
        let hash_absent = hash_absent.map(|e| builder.mul(e, not_both_present.target));

        let summary_hash = [0, 1, 2, 3].map(|i| builder.add(hash_both[i], hash_absent[i]));

        builder.register_public_input(summary_hash_present.target);
        builder.register_public_inputs(&summary_hash);

        let (circuit, r) = build(builder);

        let targets = BranchTargets {
            left_dir,
            right_dir,
            summary_hash_present,
            summary_hash: HashOutTarget::from(summary_hash),
        };
        let indices = PublicIndices {
            summary_hash_present: circuit
                .prover_only
                .public_inputs
                .iter()
                .position(|&pi| pi == targets.summary_hash_present.target)
                .expect("target not found"),
            summary_hash: targets.summary_hash.elements.map(|target| {
                circuit
                    .prover_only
                    .public_inputs
                    .iter()
                    .position(|&pi| pi == target)
                    .expect("target not found")
            }),
        };
        let v = Self {
            targets,
            indices,
            height,
            inner_circuit,
        };

        (circuit, (v, r))
    }

    fn dir_from_node(
        proof: &ProofWithPublicInputsTarget<D>,
        sub_circuit: &dyn SubCircuit<PublicIndices>,
    ) -> BranchDirTargets<D> {
        let node_idx = sub_circuit.get_indices();

        let summary_hash_present = node_idx.get_summary_hash_present(&proof.public_inputs);
        let summary_hash_present = BoolTarget::new_unsafe(summary_hash_present);
        let summary_hash = HashOutTarget::from(node_idx.get_summary_hash(&proof.public_inputs));

        BranchDirTargets {
            summary_hash_present,
            summary_hash,
        }
    }

    pub fn from_leaf<F, C, B, R>(
        builder: CircuitBuilder<F, D>,
        leaf: &'a LeafSubCircuit,
        left_proof: &ProofWithPublicInputsTarget<D>,
        right_proof: &ProofWithPublicInputsTarget<D>,
        build: B,
    ) -> (CircuitData<F, C, D>, (Self, R))
    where
        B: FnOnce(CircuitBuilder<F, D>) -> (CircuitData<F, C, D>, R),
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F>, {
        let left_dir = Self::dir_from_node(left_proof, leaf);
        let right_dir = Self::dir_from_node(right_proof, leaf);
        let height = 0;
        Self::from_dirs(leaf, builder, left_dir, right_dir, height, build)
    }

    pub fn from_branch<F, C, B, R>(
        builder: CircuitBuilder<F, D>,
        branch: &'a BranchSubCircuit<'a, D>,
        left_proof: &ProofWithPublicInputsTarget<D>,
        right_proof: &ProofWithPublicInputsTarget<D>,
        build: B,
    ) -> (CircuitData<F, C, D>, (Self, R))
    where
        B: FnOnce(CircuitBuilder<F, D>) -> (CircuitData<F, C, D>, R),
        F: RichField + Extendable<D>,
        C: GenericConfig<D, F = F>, {
        let left_dir = Self::dir_from_node(left_proof, branch);
        let right_dir = Self::dir_from_node(right_proof, branch);
        let height = branch.height + 1;
        Self::from_dirs(branch, builder, left_dir, right_dir, height, build)
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

impl<'a, const D: usize> SubCircuit<PublicIndices> for BranchSubCircuit<'a, D> {
    fn pis(&self) -> usize { 5 }

    fn get_indices(&self) -> PublicIndices { self.indices }
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use plonky2::field::types::Field;
    use plonky2::plonk::circuit_data::CircuitConfig;
    use plonky2::plonk::proof::ProofWithPublicInputs;

    use super::*;
    use crate::recproof::test::{hash_branch, hash_str};
    use crate::test_utils::{C, D, F};

    pub struct DummyLeafCircuit {
        pub summarized: LeafSubCircuit,
        pub circuit: CircuitData<F, C, D>,
    }

    impl DummyLeafCircuit {
        #[must_use]
        pub fn new(circuit_config: &CircuitConfig) -> Self {
            let builder = CircuitBuilder::<F, D>::new(circuit_config.clone());
            let (circuit, (summarized, ())) =
                LeafSubCircuit::new(builder, (), |(), _targets, builder| (builder.build(), ()));

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

    pub struct DummyBranchCircuit<'a> {
        pub summarized: BranchSubCircuit<'a, D>,
        pub circuit: CircuitData<F, C, D>,
        pub targets: DummyBranchTargets,
    }

    pub struct DummyBranchTargets {
        pub left_proof: ProofWithPublicInputsTarget<D>,
        pub right_proof: ProofWithPublicInputsTarget<D>,
    }

    impl<'a> DummyBranchCircuit<'a> {
        #[must_use]
        pub fn from_leaf(circuit_config: &CircuitConfig, leaf: &'a DummyLeafCircuit) -> Self {
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
                |builder| (builder.build(), ()),
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

        pub fn from_branch(circuit_config: &CircuitConfig, branch: &'a DummyBranchCircuit) -> Self {
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
                |builder| (builder.build(), ()),
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

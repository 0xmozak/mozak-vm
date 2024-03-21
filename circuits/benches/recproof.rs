use std::time::Duration;

use anyhow::Result;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use mozak_circuits::recproof::state_update::{BranchCircuit, LeafCircuit};
use mozak_circuits::recproof::{make_tree, unbounded};
use mozak_circuits::test_utils::{hash_branch, hash_str, C, D, F};
use plonky2::field::types::Field;
use plonky2::hash::hash_types::{HashOut, RichField};
use plonky2::hash::poseidon2::Poseidon2Hash;
use plonky2::iop::witness::PartialWitness;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::{CircuitConfig, CircuitData};
use plonky2::plonk::config::Hasher;
use plonky2::plonk::proof::ProofWithPublicInputs;

pub struct DummyLeafCircuit {
    pub unbounded: unbounded::LeafSubCircuit,
    pub make_tree: make_tree::LeafSubCircuit,
    pub circuit: CircuitData<F, C, D>,
}

impl DummyLeafCircuit {
    #[must_use]
    pub fn new(circuit_config: &CircuitConfig) -> Self {
        let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());

        let unbounded_inputs = unbounded::SubCircuitInputs::default(&mut builder);
        let make_tree_inputs = make_tree::SubCircuitInputs::default(&mut builder);

        let unbounded_targets = unbounded_inputs.build_leaf::<F, C, D>(&mut builder);
        let make_tree_targets = make_tree_inputs.build_leaf(&mut builder);

        let circuit = builder.build();

        let unbounded = unbounded_targets.build(&circuit.prover_only.public_inputs);
        let make_tree = make_tree_targets.build(&circuit.prover_only.public_inputs);

        Self {
            make_tree,
            unbounded,
            circuit,
        }
    }

    pub fn prove(
        &self,
        present: bool,
        leaf_value: HashOut<F>,
        branch: &DummyBranchCircuit,
    ) -> Result<ProofWithPublicInputs<F, C, D>> {
        let mut inputs = PartialWitness::new();
        self.make_tree.set_witness(&mut inputs, present, leaf_value);
        self.unbounded.set_witness(&mut inputs, &branch.circuit);
        self.circuit.prove(inputs)
    }
}

pub struct DummyBranchCircuit {
    pub unbounded: unbounded::BranchSubCircuit<D>,
    pub make_tree: make_tree::BranchSubCircuit,
    pub circuit: CircuitData<F, C, D>,
}

impl DummyBranchCircuit {
    #[must_use]
    pub fn new(circuit_config: &CircuitConfig, leaf: &DummyLeafCircuit) -> Self {
        let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());

        let unbounded_inputs = unbounded::SubCircuitInputs::default(&mut builder);
        let make_tree_inputs = make_tree::SubCircuitInputs::default(&mut builder);

        let unbounded_targets =
            unbounded_inputs.build_branch(&mut builder, &leaf.unbounded, &leaf.circuit);
        let make_tree_targets = make_tree_inputs.build_branch(
            &mut builder,
            &leaf.make_tree.indices,
            &unbounded_targets.left_proof,
            &unbounded_targets.right_proof,
        );

        builder.connect(
            unbounded_targets.left_is_leaf.target,
            make_tree_targets.left_is_leaf.target,
        );
        builder.connect(
            unbounded_targets.right_is_leaf.target,
            make_tree_targets.right_is_leaf.target,
        );

        let circuit = builder.build();

        let unbounded =
            unbounded_targets.build(&leaf.unbounded, &circuit.prover_only.public_inputs);
        let make_tree =
            make_tree_targets.build(&leaf.make_tree.indices, &circuit.prover_only.public_inputs);

        Self {
            make_tree,
            unbounded,
            circuit,
        }
    }

    pub fn prove(
        &self,
        hash: HashOut<F>,
        leaf_value: HashOut<F>,
        left_proof: &ProofWithPublicInputs<F, C, D>,
        right_proof: &ProofWithPublicInputs<F, C, D>,
    ) -> Result<ProofWithPublicInputs<F, C, D>> {
        let mut inputs = PartialWitness::new();
        self.unbounded
            .set_partial_witness(&mut inputs, left_proof, right_proof);
        self.make_tree.set_witness(&mut inputs, hash, leaf_value);
        self.circuit.prove(inputs)
    }
}

fn hash_write<F: RichField>(address: u64, left: &HashOut<F>, right: &HashOut<F>) -> HashOut<F> {
    let address = F::from_canonical_u64(address);
    let [l0, l1, l2, l3] = left.elements;
    let [r0, r1, r2, r3] = right.elements;
    Poseidon2Hash::hash_no_pad(&[address, l0, l1, l2, l3, r0, r1, r2, r3])
}

fn bench_prove_verify_recproof(c: &mut Criterion) {
    let mut group = c.benchmark_group("prove_verify_recproof");
    group.measurement_time(Duration::new(10, 0));

    let circuit_config = CircuitConfig::standard_recursion_config();
    let leaf_circuit = black_box(LeafCircuit::<F, C, D>::new(&circuit_config));
    let branch_circuit_1 = BranchCircuit::from_leaf(&circuit_config, &leaf_circuit);
    let branch_circuit_2 = BranchCircuit::from_branch(&circuit_config, &branch_circuit_1);

    let zero_hash = black_box(HashOut::from([F::ZERO; 4]));
    let non_zero_hash_1 = black_box(hash_str("Non-Zero Hash 1"));
    let hash_0_and_0 = black_box(hash_branch(&zero_hash, &zero_hash));
    let hash_0_and_1 = black_box(hash_branch(&zero_hash, &non_zero_hash_1));
    let hash_1_and_0 = hash_branch(&non_zero_hash_1, &zero_hash);
    let hash_00_and_00 = hash_branch(&hash_0_and_0, &hash_0_and_0);
    let hash_01_and_10 = hash_branch(&hash_0_and_1, &hash_1_and_0);

    let slot_3_r0w1 = hash_write(3, &zero_hash, &non_zero_hash_1);
    let slot_4_r0w1 = hash_write(4, &zero_hash, &non_zero_hash_1);
    let slot_3_and_4 = hash_branch(&slot_3_r0w1, &slot_4_r0w1);

    // Leaf proofs
    let zero_proof = leaf_circuit
        .prove(zero_hash, zero_hash, zero_hash, None)
        .unwrap();
    leaf_circuit.circuit.verify(zero_proof.clone()).unwrap();

    let proof_0_to_1_id_3 = leaf_circuit
        .prove(zero_hash, non_zero_hash_1, slot_3_r0w1, Some(3))
        .unwrap();
    leaf_circuit
        .circuit
        .verify(proof_0_to_1_id_3.clone())
        .unwrap();

    let proof_0_to_1_id_4 = leaf_circuit
        .prove(zero_hash, non_zero_hash_1, slot_4_r0w1, Some(4))
        .unwrap();
    leaf_circuit
        .circuit
        .verify(proof_0_to_1_id_4.clone())
        .unwrap();

    // Branch proofs
    let branch_00_and_01_proof = branch_circuit_1
        .prove(
            hash_0_and_0,
            hash_0_and_1,
            slot_3_r0w1,
            (),
            &zero_proof,
            &proof_0_to_1_id_3,
        )
        .unwrap();

    let branch_01_and_00_proof = branch_circuit_1
        .prove(
            hash_0_and_0,
            hash_1_and_0,
            slot_4_r0w1,
            (),
            &proof_0_to_1_id_4,
            &zero_proof,
        )
        .unwrap();

    // Benches
    group.bench_function("recproof_leaf_prove", |b| {
        b.iter(|| {
            leaf_circuit
                .prove(zero_hash, non_zero_hash_1, slot_3_r0w1, Some(3))
                .unwrap()
        })
    });
    group.bench_function("recproof_leaf_verify", |b| {
        b.iter(|| {
            leaf_circuit
                .circuit
                .verify(proof_0_to_1_id_3.clone())
                .unwrap()
        })
    });

    group.bench_function("recproof_branch_prove_1", |b| {
        b.iter(|| {
            branch_circuit_1
                .prove(
                    hash_0_and_0,
                    hash_0_and_1,
                    slot_3_r0w1,
                    (),
                    &zero_proof,
                    &proof_0_to_1_id_3,
                )
                .unwrap()
        })
    });
    group.bench_function("recproof_branch_verify", |b| {
        b.iter(|| {
            branch_circuit_1
                .circuit
                .verify(branch_00_and_01_proof.clone())
                .unwrap()
        })
    });

    group.bench_function("recproof_branch_prove_2", |b| {
        b.iter(|| {
            branch_circuit_2
                .prove(
                    hash_00_and_00,
                    hash_01_and_10,
                    slot_3_and_4,
                    (),
                    &branch_00_and_01_proof,
                    &branch_01_and_00_proof,
                )
                .unwrap()
        })
    });

    group.finish();
}

fn bench_prove_verify_unbounded(c: &mut Criterion) {
    let mut group = c.benchmark_group("prove_verify_unbounded");
    group.measurement_time(Duration::new(10, 0));

    let circuit_config = CircuitConfig::standard_recursion_config();
    let leaf = black_box(DummyLeafCircuit::new(&circuit_config));
    let branch = DummyBranchCircuit::new(&circuit_config, &leaf);

    let non_zero_hash = black_box(hash_str("Non-Zero Hash"));
    let branch_hash = hash_branch(&non_zero_hash, &non_zero_hash);
    let branch_hash_1 = hash_branch(&non_zero_hash, &branch_hash);

    let leaf_1_proof = leaf.prove(false, non_zero_hash, &branch).unwrap();
    leaf.circuit.verify(leaf_1_proof.clone()).unwrap();

    let leaf_2_proof = leaf.prove(true, non_zero_hash, &branch).unwrap();
    leaf.circuit.verify(leaf_2_proof.clone()).unwrap();

    let branch_proof_1 = branch
        .prove(non_zero_hash, non_zero_hash, &leaf_1_proof, &leaf_2_proof)
        .unwrap();
    branch.circuit.verify(branch_proof_1.clone()).unwrap();

    let branch_proof_2 = branch
        .prove(branch_hash, non_zero_hash, &leaf_2_proof, &leaf_2_proof)
        .unwrap();
    branch.circuit.verify(branch_proof_2.clone()).unwrap();

    let double_branch_proof = branch
        .prove(branch_hash_1, non_zero_hash, &leaf_2_proof, &branch_proof_2)
        .unwrap();
    branch.circuit.verify(double_branch_proof.clone()).unwrap();

    group.bench_function("branch_prove_1", |b| {
        b.iter(|| {
            branch
                .prove(non_zero_hash, non_zero_hash, &leaf_1_proof, &leaf_2_proof)
                .unwrap()
        })
    });
    group.bench_function("branch_verify_1", |b| {
        b.iter(|| branch.circuit.verify(branch_proof_1.clone()).unwrap())
    });

    group.bench_function("branch_prove_2", |b| {
        b.iter(|| {
            branch
                .prove(branch_hash_1, non_zero_hash, &leaf_2_proof, &branch_proof_2)
                .unwrap()
        })
    });
    group.bench_function("branch_verify_2", |b| {
        b.iter(|| branch.circuit.verify(double_branch_proof.clone()).unwrap())
    });

    group.finish();
}

criterion_group![
    name = benches;
    config = Criterion::default().sample_size(10);
    targets = bench_prove_verify_recproof, bench_prove_verify_unbounded
];
criterion_main!(benches);

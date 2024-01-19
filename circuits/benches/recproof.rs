use std::time::Duration;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use mozak_circuits::recproof::{CompleteBranchCircuit, CompleteLeafCircuit};
use mozak_circuits::test_utils::{hash_branch, hash_str, C, D, F};
use plonky2::field::types::Field;
use plonky2::hash::hash_types::HashOut;
use plonky2::plonk::circuit_data::CircuitConfig;

fn bench_prove_verify_recproof(c: &mut Criterion) {
    let mut group = c.benchmark_group("prove_verify_recproof");
    group.measurement_time(Duration::new(10, 0));

    let circuit_config = CircuitConfig::standard_recursion_config();
    let leaf_circuit = black_box(CompleteLeafCircuit::<F, C, D>::new(&circuit_config));
    let branch_circuit_1 = CompleteBranchCircuit::from_leaf(&circuit_config, &leaf_circuit);
    let branch_circuit_2 = CompleteBranchCircuit::from_branch(&circuit_config, &branch_circuit_1);

    let zero_hash = black_box(HashOut::from([F::ZERO; 4]));
    let non_zero_hash_1 = black_box(hash_str("Non-Zero Hash 1"));
    let hash_0_and_0 = black_box(hash_branch(&zero_hash, &zero_hash));
    let hash_0_and_1 = black_box(hash_branch(&zero_hash, &non_zero_hash_1));
    let hash_1_and_0 = hash_branch(&non_zero_hash_1, &zero_hash);
    let hash_00_and_00 = hash_branch(&hash_0_and_0, &hash_0_and_0);
    let hash_01_and_01 = hash_branch(&hash_0_and_1, &hash_0_and_1);
    let hash_01_and_10 = hash_branch(&hash_0_and_1, &hash_1_and_0);

    // Leaf proofs
    let zero_proof = leaf_circuit.prove(zero_hash, zero_hash, zero_hash).unwrap();
    leaf_circuit.circuit.verify(zero_proof.clone()).unwrap();

    let proof_0_to_1 = leaf_circuit
        .prove(zero_hash, non_zero_hash_1, hash_0_and_1)
        .unwrap();
    leaf_circuit.circuit.verify(proof_0_to_1.clone()).unwrap();

    // Branch proofs
    let branch_00_and_01_proof = branch_circuit_1
        .prove(
            &zero_proof,
            &proof_0_to_1,
            hash_0_and_0,
            hash_0_and_1,
            hash_0_and_1,
        )
        .unwrap();

    let branch_01_and_00_proof = branch_circuit_1
        .prove(
            &proof_0_to_1,
            &zero_proof,
            hash_0_and_0,
            hash_1_and_0,
            hash_0_and_1,
        )
        .unwrap();

    // Benches
    group.bench_function("recproof_leaf_prove", |b| {
        b.iter(|| {
            leaf_circuit
                .prove(zero_hash, non_zero_hash_1, hash_0_and_1)
                .unwrap()
        })
    });
    group.bench_function("recproof_leaf_verify", |b| {
        b.iter(|| leaf_circuit.circuit.verify(proof_0_to_1.clone()).unwrap())
    });

    group.bench_function("recproof_branch_prove_1", |b| {
        b.iter(|| {
            branch_circuit_1
                .prove(
                    &zero_proof,
                    &proof_0_to_1,
                    hash_0_and_0,
                    hash_0_and_1,
                    hash_0_and_1,
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
                    &branch_00_and_01_proof,
                    &branch_01_and_00_proof,
                    hash_00_and_00,
                    hash_01_and_10,
                    hash_01_and_01,
                )
                .unwrap()
        })
    });

    group.finish();
}

criterion_group![
    name = benches;
    config = Criterion::default().sample_size(10);
    targets = bench_prove_verify_recproof
];
criterion_main!(benches);

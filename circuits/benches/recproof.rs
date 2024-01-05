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

    let zero_hash = black_box(HashOut::from([F::ZERO; 4]));
    let non_zero_hash_1 = black_box(hash_str("Non-Zero Hash 1"));
    let hash_0_to_0 = black_box(hash_branch(&zero_hash, &zero_hash));
    let hash_0_to_1 = black_box(hash_branch(&zero_hash, &non_zero_hash_1));

    group.bench_function("recproof_leaf_prove", |b| {
        b.iter(|| {
            leaf_circuit
                .prove(zero_hash, non_zero_hash_1, hash_0_to_1)
                .unwrap()
        })
    });

    group.bench_function("recproof_leaf_verify", |b| {
        let leaf_proof = leaf_circuit
            .prove(zero_hash, non_zero_hash_1, hash_0_to_1)
            .unwrap();
        b.iter(|| leaf_circuit.circuit.verify(leaf_proof.clone()).unwrap())
    });

    // Leaf proofs
    let zero_proof = leaf_circuit.prove(zero_hash, zero_hash, zero_hash).unwrap();
    leaf_circuit.circuit.verify(zero_proof.clone()).unwrap();

    let proof_0_to_1 = leaf_circuit
        .prove(zero_hash, non_zero_hash_1, hash_0_to_1)
        .unwrap();
    leaf_circuit.circuit.verify(proof_0_to_1.clone()).unwrap();

    group.bench_function("recproof_branch_prove_1", |b| {
        b.iter(|| {
            branch_circuit_1
                .prove(
                    &zero_proof,
                    &proof_0_to_1,
                    hash_0_to_0,
                    hash_0_to_1,
                    hash_0_to_1,
                )
                .unwrap()
        })
    });
    group.bench_function("recproof_branch_verify", |b| {
        let branch_00_and_01_proof = branch_circuit_1
            .prove(
                &zero_proof,
                &proof_0_to_1,
                hash_0_to_0,
                hash_0_to_1,
                hash_0_to_1,
            )
            .unwrap();

        b.iter(|| {
            branch_circuit_1
                .circuit
                .verify(branch_00_and_01_proof.clone())
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

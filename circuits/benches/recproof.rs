// use std::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion};
// use criterion::{black_box, criterion_group, criterion_main, Criterion};
// use mozak_circuits::recproof::{hash_branch, BranchCircuit, LeafCircuit,
// Object}; use mozak_circuits::test_utils::{C, D, F};
// use plonky2::field::types::{Field, Sample};
// use plonky2::hash::hash_types::HashOut;
// use plonky2::hash::poseidon2::Poseidon2Hash;
// use plonky2::plonk::circuit_data::CircuitConfig;
// use plonky2::plonk::config::Hasher;

// fn hash_str(v: &str) -> HashOut<F> {
//     let v: Vec<_> = v.bytes().map(F::from_canonical_u8).collect();
//     Poseidon2Hash::hash_no_pad(&v)
// }

fn bench_prove_verify_recproof(_c: &mut Criterion) {
    // let mut group = c.benchmark_group("prove_verify_recproof");
    // group.measurement_time(Duration::new(10, 0));

    // let data_a_v1 = F::rand_array();
    // let data_a_v2 = F::rand_array();
    // let data_b = F::rand_array();

    // let owner_a = hash_str("Totally A Program");
    // let owner_b = hash_str("A Different Program");

    // let circuit_config = CircuitConfig::standard_recursion_config();
    // let leaf_circuit = black_box(LeafCircuit::<F, C,
    // D>::new(&circuit_config));

    // let object_a_v1 = black_box(Object::<F, D> {
    //     data: data_a_v1,
    //     last_updated: F::from_canonical_u8(42),
    //     lifetime: F::from_canonical_u8(69),
    //     owner: owner_a,
    // });
    // let object_a_v2 = black_box(Object::<F, D> {
    //     data: data_a_v2,
    //     last_updated: F::from_canonical_u8(44),
    //     lifetime: F::from_canonical_u8(69),
    //     owner: owner_a,
    // });
    // let object_a_v1_pair = (&object_a_v1, &object_a_v1.hash());
    // let object_a_v2_pair = (&object_a_v2, &object_a_v2.hash());

    // // Update an object
    // let leaf_proof = leaf_circuit
    //     .prove(Some(object_a_v1_pair), Ok(object_a_v2_pair))
    //     .unwrap();
    // leaf_circuit.circuit.verify(leaf_proof.clone()).unwrap();

    // // No updates
    // let object_b = black_box(Object::<F, D> {
    //     data: data_b,
    //     last_updated: F::from_canonical_u8(43),
    //     lifetime: F::from_canonical_u8(99),
    //     owner: owner_b,
    // });
    // let object_b_pair = (&object_b, &object_b.hash());

    // // Branch
    // let branch_circuit = black_box(BranchCircuit::<F, C, D>::from_leaf(
    //     &circuit_config,
    //     &leaf_circuit,
    // ));

    // let branch_ab = black_box(hash_branch(object_a_v1_pair.1,
    // object_b_pair.1)); let branch_ab_v2 =
    // black_box(hash_branch(object_a_v2_pair.1, object_b_pair.1));

    // let branch_proof = branch_circuit
    //     .prove(
    //         Some(object_a_v1_pair.1),
    //         Some(&leaf_proof),
    //         Some(object_b_pair.1),
    //         None,
    //         Some(&branch_ab),
    //         Some(&branch_ab_v2),
    //     )
    //     .unwrap();
    // branch_circuit.circuit.verify(branch_proof.clone()).unwrap();

    // group.bench_function("recproof_leaf_prove", |b| {
    //     b.iter(|| {
    //         leaf_circuit
    //             .prove(Some(object_a_v1_pair), Ok(object_a_v2_pair))
    //             .unwrap()
    //     })
    // });
    // group.bench_function("recproof_leaf_verify", |b| {
    //     b.iter(|| leaf_circuit.circuit.verify(leaf_proof.clone()).unwrap())
    // });
    // group.bench_function("recproof_branch_prove_1", |b| {
    //     b.iter(|| {
    //         branch_circuit
    //             .prove(
    //                 Some(object_a_v1_pair.1),
    //                 Some(&leaf_proof),
    //                 Some(object_b_pair.1),
    //                 None,
    //                 Some(&branch_ab),
    //                 Some(&branch_ab_v2),
    //             )
    //             .unwrap()
    //     })
    // });
    // group.bench_function("recproof_branch_verify", |b| {
    //     b.iter(||
    // branch_circuit.circuit.verify(branch_proof.clone()).unwrap()) });
    // group.finish();
}

criterion_group![
    name = benches;
    config = Criterion::default().sample_size(10);
    targets = bench_prove_verify_recproof
];
criterion_main!(benches);

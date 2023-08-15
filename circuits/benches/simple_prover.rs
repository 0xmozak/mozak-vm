use std::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion};
use mozak_circuits::stark::prover::prove;
use mozak_circuits::stark::verifier::verify_proof;
use mozak_circuits::test_utils::{standard_faster_config, C, D, F, S};
use mozak_vm::test_utils::simple_test_code;
use plonky2::field::types::Field;
use plonky2::util::timing::TimingTree;

pub(crate) fn bench_simple() {
    let (program, record) = simple_test_code(&[], &[], &[]);
    let stark = S::default();
    let config = standard_faster_config();

    let all_proof = prove::<F, C, D>(
        &program,
        &record,
        &stark,
        &config,
        [F::ZERO],
        &mut TimingTree::default(),
    )
    .unwrap();
    verify_proof(stark, all_proof, &config).unwrap();
}

fn simple_benchmark(c: &mut Criterion) {
    let _ = env_logger::builder().try_init();
    let mut group = c.benchmark_group("simple_prover");
    group.measurement_time(Duration::new(10, 0));
    group.bench_function("simple_prover", |b| {
        b.iter(|| {
            bench_simple();
        })
    });
    group.finish();
}

criterion_group![
    name = benches;
    config = Criterion::default().sample_size(10);
    targets = simple_benchmark
];
criterion_main!(benches);

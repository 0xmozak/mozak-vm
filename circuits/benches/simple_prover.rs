use std::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion};
use mozak_circuits::stark::mozak_stark::MozakStark;
use mozak_circuits::test_utils::ProveAndVerify;
use mozak_vm::test_utils::simple_test_code;

fn bench_prove_verify_all(c: &mut Criterion) {
    let _ = env_logger::builder().try_init();
    let mut group = c.benchmark_group("prove_verify_all");
    group.measurement_time(Duration::new(10, 0));
    group.bench_function("prove_verify_all", |b| {
        b.iter(|| {
            let (program, record) = simple_test_code(&[], &[], &[]);
            MozakStark::prove_and_verify(&program, &record)
        })
    });
    group.finish();
}

criterion_group![
    name = benches;
    config = Criterion::default().sample_size(10);
    targets = bench_prove_verify_all
];
criterion_main!(benches);

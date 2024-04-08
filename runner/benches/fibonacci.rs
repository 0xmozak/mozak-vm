use std::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion};
use mozak_examples::FIBONACCI_ELF;
use mozak_runner::elf::Program;
use mozak_runner::state::State;
use mozak_runner::vm::step;
use plonky2::field::goldilocks_field::GoldilocksField;

fn fibonacci_benchmark(c: &mut Criterion) {
    let _ = env_logger::builder().try_init();
    let mut group = c.benchmark_group("fibonacci");
    group.measurement_time(Duration::new(10, 0));
    group.bench_function("fibonacci", |b| {
        b.iter(|| {
            let program = Program::vanilla_load_elf(FIBONACCI_ELF).unwrap();
            let state = State::<GoldilocksField>::from(&program);
            let _state = step(&program, state).unwrap();
        })
    });
    group.finish();
}

criterion_group![
    name = benches;
    config = Criterion::default().sample_size(10);
    targets = fibonacci_benchmark
];
criterion_main!(benches);

use std::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion};
use mozak_runner::elf::Program;
use mozak_runner::state::State;
use mozak_runner::vm::step;

pub(crate) fn bench_fibonacci() {
    let elf = std::fs::read("benches/fibonacci.elf").unwrap();
    let program = Program::load_elf(&elf).unwrap();
    let state = State::from(&program);
    let _state = step(&program, state).unwrap();
}

fn fibonacci_benchmark(c: &mut Criterion) {
    let _ = env_logger::builder().try_init();
    let mut group = c.benchmark_group("fibonacci");
    group.measurement_time(Duration::new(10, 0));
    group.bench_function("fibonacci", |b| {
        b.iter(|| {
            bench_fibonacci();
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

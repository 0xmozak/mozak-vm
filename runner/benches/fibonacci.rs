use std::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion};
use mozak_runner::elf::Program;
use mozak_runner::state::State;
use mozak_runner::vm::step;

const FIBO_ELF_EXAMPLE_PATH: &str = "examples/target/riscv32im-mozak-zkvm-elf/release/fibonacci";

fn fibonacci_benchmark(c: &mut Criterion) {
    let elf_path = std::env::current_dir()
        .unwrap()
        .parent()
        .unwrap()
        .join(FIBO_ELF_EXAMPLE_PATH);
    let elf = std::fs::read(elf_path).expect(
        "Reading the fibonacci elf should not fail.
        You may need to build the fibonacci program within the examples directory
        eg. `cd examples/fibonacci && cargo build --release`",
    );

    let _ = env_logger::builder().try_init();
    let mut group = c.benchmark_group("fibonacci");
    group.measurement_time(Duration::new(10, 0));
    group.bench_function("fibonacci", |b| {
        b.iter(|| {
            let program = Program::load_elf(&elf).unwrap();
            let state = State::from(&program);
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

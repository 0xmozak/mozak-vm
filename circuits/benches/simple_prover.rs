use std::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion};
use mozak_circuits::test_utils::prove_and_verify_mozak_stark;
use mozak_runner::instruction::{Args, Instruction, Op};
use mozak_runner::test_utils::execute_code;
use starky::config::StarkConfig;

fn bench_prove_verify_all(c: &mut Criterion) {
    let _ = env_logger::builder().try_init();
    let mut group = c.benchmark_group("prove_verify_all");
    group.measurement_time(Duration::new(10, 0));
    group.bench_function("prove_verify_all", |b| {
        b.iter(|| {
            let instructions = [
                Instruction {
                    op: Op::ADD,
                    args: Args {
                        rd: 1,
                        rs1: 1,
                        imm: 1_u32.wrapping_neg(),
                        ..Args::default()
                    },
                },
                Instruction {
                    op: Op::BLT,
                    args: Args {
                        rs1: 0,
                        rs2: 1,
                        imm: 0,
                        ..Args::default()
                    },
                },
            ];
            let (program, record) = execute_code(instructions, &[], &[(1, 1 << 10)]);
            prove_and_verify_mozak_stark(&program, &record, &StarkConfig::standard_fast_config())
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

use std::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion};
use mozak_circuits::stark::mozak_stark::MozakStark;
use mozak_circuits::test_utils::ProveAndVerify;
use mozak_runner::instruction::{Args, Instruction, Op};
use mozak_runner::test_utils::simple_test_code;

fn bench_prove_verify_all(c: &mut Criterion) {
    let _ = env_logger::builder().try_init();
    let mut group = c.benchmark_group("prove_verify_all");
    group.measurement_time(Duration::new(10, 0));
    group.bench_function("prove_verify_all", |b| {
        b.iter(|| {
            let instructions = &[
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
            let (program, record) = simple_test_code(instructions, &[], &[(1, 1 << 16)]);
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

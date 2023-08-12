use std::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion};
use mozak_circuits::stark::prover::prove;
use mozak_circuits::stark::verifier::verify_proof;
use mozak_circuits::test_utils::{standard_faster_config, C, D, F, S};
use mozak_vm::test_utils::simple_test_code;
use plonky2::util::timing::TimingTree;
use mozak_vm::instruction::{Args, Instruction, Op};

pub(crate) fn bench_simple() {
    let loops = 100_000;
    // TODO: use a counter and a branch to jump back?
    let code = vec![
        Instruction {
            op: Op::ADD,
            args: Args {
                rd: 1,
                imm: loops,
                ..Args::default()
            },
        },
        Instruction {
            op: Op::AND,
            args: Args {
                rd: 8,
                rs1: 1,
                imm: 0xDEAD_BEEF,
                ..Args::default()
            },
        },
        Instruction {
            op: Op::SUB,
            args: Args {
                rd: 1,
                rs1: 1,
                imm: 1,
                ..Args::default()
            },
        },
        Instruction {
            op: Op::BLT,
            args: Args {
                rs1: 0,
                rs2: 1,
                branch_target: 4,
                ..Args::default()
            },
        },
    ];

    let (program, record) = simple_test_code(&code, &[], &[]);
    let stark = S::default();
    let config = standard_faster_config();

    let all_proof = prove::<F, C, D>(
        &program,
        &record.executed,
        &stark,
        &config,
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
    // config = Criterion::default().sample_size(10);
    config = Criterion::default().sample_size(10).measurement_time(Duration::from_secs(150));
    targets = simple_benchmark
];
criterion_main!(benches);

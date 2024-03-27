use mozak_circuits::test_utils::prove_and_verify_mozak_stark_with_timing;
use mozak_runner::instruction::{Args, Instruction, Op, NOP};
use mozak_runner::util::execute_code;
use plonky2::timed;
use plonky2::util::timing::TimingTree;
use starky::config::StarkConfig;

#[allow(clippy::module_name_repetitions)]
pub fn nop_bench(timing: &mut TimingTree, iterations: u32) -> Result<(), anyhow::Error> {
    let instructions = [
        Instruction {
            op: Op::ADD,
            args: Args {
                rd: 1,
                rs2: 1,
                imm: 1_u32.wrapping_neg(),
                ..Args::default()
            },
        },
        NOP,
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
    let (program, record) = timed!(
        timing,
        "nop_bench_execution",
        execute_code(instructions, &[], &[(1, iterations)])
    );

    timed!(
        timing,
        "nop bench prove_and_verify_mozak_stark_with_timing",
        prove_and_verify_mozak_stark_with_timing(
            timing,
            &program,
            &record,
            &StarkConfig::standard_fast_config(),
        )
    )
}

#[cfg(test)]
mod tests {
    use plonky2::util::timing::TimingTree;

    use crate::cli_benches::benches::{BenchArgs, BenchFunction};

    #[test]
    fn test_nop_bench() {
        let iterations = 10;
        super::nop_bench(&mut TimingTree::default(), iterations).unwrap();
    }

    #[test]
    fn test_nop_bench_with_run() {
        let iterations = 10;
        let bench = BenchArgs {
            function: BenchFunction::NopBench { iterations },
        };
        bench.run_with_default_timing().unwrap();
    }
}

use mozak_circuits::test_utils::prove_and_verify_mozak_stark;
use mozak_runner::instruction::{Args, Instruction, Op, NOP};
use mozak_runner::util::execute_code;
use starky::config::StarkConfig;

#[allow(clippy::module_name_repetitions)]
pub fn nop_bench(iterations: u32) -> Result<(), anyhow::Error> {
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
    let (program, record) = execute_code(instructions, &[], &[(1, iterations)]);
    prove_and_verify_mozak_stark(&program, &record, &StarkConfig::standard_fast_config())
}

#[cfg(test)]
mod tests {
    use crate::cli_benches::benches::{BenchArgs, BenchFunction};

    #[test]
    fn test_nop_bench() {
        let iterations = 10;
        super::nop_bench(iterations).unwrap();
    }

    #[test]
    fn test_nop_bench_with_run() {
        let iterations = 10;
        let bench = BenchArgs {
            function: BenchFunction::NopBench { iterations },
        };
        bench.run().unwrap();
    }
}

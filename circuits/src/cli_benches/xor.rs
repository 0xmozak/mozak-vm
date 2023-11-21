use mozak_runner::instruction::{Args, Instruction, Op, NOP};
use mozak_runner::test_utils::simple_test_code;
use starky::config::StarkConfig;

use crate::test_utils::prove_and_verify_mozak_stark;

pub fn xor_bench(n: u32) -> Result<(), anyhow::Error> {
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
    let (program, record) = simple_test_code(instructions, &[], &[(1, n)]);
    prove_and_verify_mozak_stark(&program, &record, &StarkConfig::standard_fast_config())
}

#[cfg(test)]
mod tests {

    use crate::cli_benches::{BenchArgs, BenchFunction};

    #[test]
    fn test_xor_bench() {
        let n = 10;
        super::xor_bench(n).unwrap();
    }

    #[test]
    fn test_xor_bench_with_run() {
        let iterations = 10;
        let bench = BenchArgs {
            function: BenchFunction::XorBench { iterations },
        };
        bench.run().unwrap();
    }
}

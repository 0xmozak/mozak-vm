use mozak_runner::instruction::{Args, Instruction, Op};
use mozak_runner::test_utils::simple_test_code;

use crate::stark::mozak_stark::MozakStark;
use crate::test_utils::ProveAndVerify;

pub fn sample_bench(reg_value: u32) -> Result<(), anyhow::Error> {
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
    let (program, record) = simple_test_code(instructions, &[], &[(1, reg_value)]);
    MozakStark::prove_and_verify(&program, &record)
}

#[cfg(test)]
mod tests {
    use crate::cli_benches::{BenchArgs, BenchFunction};

    #[test]
    fn test_sample_bench() { super::sample_bench(123).unwrap(); }

    #[test]
    fn test_sample_bench_run() {
        let bench = BenchArgs {
            function: BenchFunction::SampleBench { iterations: 123 },
        };
        bench.run().unwrap();
    }
}

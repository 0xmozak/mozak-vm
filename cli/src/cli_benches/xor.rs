use mozak_circuits::test_utils::prove_and_verify_mozak_stark;
use mozak_runner::instruction::{Args, Instruction, Op};
use mozak_runner::util::code::execute;
use starky::config::StarkConfig;

#[allow(clippy::module_name_repetitions)]
pub fn xor_bench(iterations: u32) -> Result<(), anyhow::Error> {
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
            op: Op::XOR,
            args: Args {
                rd: 2,
                rs1: 1,
                imm: 0xDEAD_BEEF,
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
    let (program, record) = code::execute(instructions, &[], &[(1, iterations)]);
    prove_and_verify_mozak_stark(&program, &record, &StarkConfig::standard_fast_config())
}

#[cfg(test)]
mod tests {
    use crate::cli_benches::benches::{BenchArgs, BenchFunction};

    #[test]
    fn test_xor_bench() {
        let iterations = 10;
        super::xor_bench(iterations).unwrap();
    }

    #[test]
    fn test_xor_bench_with_run() {
        let iterations = 10;
        let function = BenchFunction::XorBench { iterations };
        let bench = BenchArgs { function };
        bench.run().unwrap();
    }
}

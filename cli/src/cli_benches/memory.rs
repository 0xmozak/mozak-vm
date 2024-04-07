use mozak_circuits::test_utils::prove_and_verify_mozak_stark;
use mozak_runner::instruction::{Args, Instruction, Op};
use mozak_runner::util::execute_code;
use starky::config::StarkConfig;

// Stick some byte and word (and half-word?) memory operations in a big old
// loop. Do some randomisation?
//
// Code it up in Rust?  Or as assembly?
//
// just use two counters.  Outer counter, and inner counter.
//
// r1: counter
// r3: memory value
//

#[allow(clippy::module_name_repetitions)]
pub fn memory_bench(iterations: u32) -> Result<(), anyhow::Error> {
    let instructions = [
        Instruction {
            op: Op::SW,
            args: Args {
                rs1: 1,
                rs2: 1,
                imm: 0xDEAD_BEEF,
                ..Args::default()
            },
        },
        Instruction {
            op: Op::LW,
            args: Args {
                rd: 1,
                rs2: 1,
                imm: 0xDEAD_BEEF,
                ..Args::default()
            },
        },
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
            op: Op::BLTU,
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
    fn test_memory_bench() {
        let iterations = 10;
        super::memory_bench(iterations).unwrap();
    }

    #[test]
    fn test_memory_bench_with_run() {
        let iterations = 10;
        let bench = BenchArgs {
            function: BenchFunction::MemoryBench { iterations },
        };
        bench.run().unwrap();
    }
}

use mozak_circuits::test_utils::{prove_and_verify_mozak_stark, F};
use mozak_runner::code;
use mozak_runner::elf::Program;
use mozak_runner::instruction::{Args, Instruction, Op};
use mozak_runner::vm::ExecutionRecord;
use starky::config::StarkConfig;

use super::benches::Bench;

#[allow(clippy::module_name_repetitions)]
pub fn xor_execute((program, record): (Program, ExecutionRecord<F>)) -> Result<(), anyhow::Error> {
    prove_and_verify_mozak_stark(&program, &record, &StarkConfig::standard_fast_config())
}

pub fn xor_prepare(iterations: u32) -> (Program, ExecutionRecord<F>) {
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
    code::execute(instructions, &[], &[(1, iterations)])
}

pub(crate) struct XorBench;

impl Bench for XorBench {
    type Args = u32;
    type Prepared = (Program, ExecutionRecord<F>);

    fn prepare(&self, args: &Self::Args) -> Self::Prepared { xor_prepare(*args) }

    fn execute(&self, prepared: Self::Prepared) -> anyhow::Result<()> { xor_execute(prepared) }
}
#[cfg(test)]
mod tests {
    use super::{xor_execute, xor_prepare};

    #[test]
    fn test_xor_bench() -> anyhow::Result<()> {
        let iterations = 10;
        xor_execute(xor_prepare(iterations))
    }
}

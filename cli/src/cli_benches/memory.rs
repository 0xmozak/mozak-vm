use anyhow::Result;
use mozak_circuits::test_utils::{prove_and_verify_mozak_stark, F};
use mozak_runner::code;
use mozak_runner::elf::Program;
use mozak_runner::instruction::{Args, Instruction, Op};
use mozak_runner::vm::ExecutionRecord;
use starky::config::StarkConfig;

use super::benches::Bench;

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

pub fn memory_execute((program, record): (Program, ExecutionRecord<F>)) -> Result<()> {
    prove_and_verify_mozak_stark(&program, &record, &StarkConfig::standard_fast_config())
}

pub fn memory_prepare(iterations: u32) -> (Program, ExecutionRecord<F>) {
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
    code::execute(instructions, &[], &[(1, iterations)])
}

pub(crate) struct MemoryBench;

impl Bench for MemoryBench {
    type Args = u32;
    type Prepared = (Program, ExecutionRecord<F>);

    fn prepare(&self, args: &Self::Args) -> Self::Prepared { memory_prepare(*args) }

    fn execute(&self, prepared: Self::Prepared) -> Result<()> { memory_execute(prepared) }
}

#[cfg(test)]
mod tests {
    use super::{memory_execute, memory_prepare};

    #[test]
    fn test_memory_bench() -> anyhow::Result<()> {
        let iterations = 10;
        memory_execute(memory_prepare(iterations))
    }
}

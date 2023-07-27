use std::collections::HashSet;
use plonky2::hash::hash_types::RichField;
use crate::cpu::columns::{self as cpu_cols};
use crate::program::columns::{MAP, NUM_PROGRAM_COLS};

/// Generates a program trace from CPU traces.
///
/// Note: The ideal source for generating the program trace should be ELF file instructions
/// instead of CPU traces. This approach would require a substantial refactoring, including
/// the separation of local opcode decoding from CPU trace generation.
#[must_use]
#[allow(clippy::missing_panics_doc)]
pub fn generate_program_trace<F: RichField>(
    cpu_trace: &[Vec<F>; cpu_cols::NUM_CPU_COLS],
) -> [Vec<F>; NUM_PROGRAM_COLS] {
    let mut unique_instructions = HashSet::new();

    for i in 0..cpu_trace[cpu_cols::MAP.pc].len() {
        let instruction = (
            cpu_trace[cpu_cols::MAP.pc][i],
            cpu_trace[cpu_cols::MAP.opcode][i],
            cpu_trace[cpu_cols::MAP.rs1][i],
            cpu_trace[cpu_cols::MAP.rs2][i],
            cpu_trace[cpu_cols::MAP.rd][i],
            cpu_trace[cpu_cols::MAP.imm_value][i],
        );
        unique_instructions.insert(instruction);
    }

    let trace_len = unique_instructions.len().next_power_of_two();
    let mut trace_res: [Vec<F>; NUM_PROGRAM_COLS] = Default::default();

    for vec in &mut trace_res {
        vec.resize(trace_len, F::ZERO);
    }

    for (i, instruction) in unique_instructions.into_iter().enumerate() {
        let (pc, opcode, rs1, rs2, rd, imm) = instruction;
        trace_res[MAP.program_is_inst][i] = F::ONE;
        trace_res[MAP.program_pc][i] = pc;
        trace_res[MAP.program_opcode][i] = opcode;
        trace_res[MAP.program_rs1][i] = rs1;
        trace_res[MAP.program_rs2][i] = rs2;
        trace_res[MAP.program_rd][i] = rd;
        trace_res[MAP.program_imm][i] = imm;
    }

    trace_res
}

use std::collections::HashSet;

use plonky2::hash::hash_types::RichField;

use crate::cpu::columns::{self as cpu_cols};
use crate::program::columns::ProgramColumnsView;

// TODO: The generation should be done directly from ELF file instructions
// instead of CPU traces. This approach would need a substantial refactoring,
// including the extraction of local opcode decoding from CPU trace generation.
#[must_use]
#[allow(clippy::missing_panics_doc)]
pub fn generate_program_trace<F: RichField>(
    cpu_trace: &[Vec<F>; cpu_cols::NUM_CPU_COLS],
) -> Vec<ProgramColumnsView<F>> {
    let mut unique_inst = HashSet::new();
    for i in 0..cpu_trace[cpu_cols::MAP.pc].len() {
        let instruction = (
            cpu_trace[cpu_cols::MAP.pc][i],
            cpu_trace[cpu_cols::MAP.opcode][i],
            cpu_trace[cpu_cols::MAP.rs1][i],
            cpu_trace[cpu_cols::MAP.rs2][i],
            cpu_trace[cpu_cols::MAP.rd][i],
            cpu_trace[cpu_cols::MAP.imm_value][i],
        );
        unique_inst.insert(instruction);
    }

    let mut trace = Vec::with_capacity(unique_inst.len());
    for inst in unique_inst {
        let (pc, opcode, rs1, rs2, rd, imm) = inst;
        let mut row = ProgramColumnsView::default();
        row.program_is_inst = F::ONE;
        row.program_pc = pc;
        row.program_opcode = opcode;
        row.program_rs1 = rs1;
        row.program_rs2 = rs2;
        row.program_rd = rd;
        row.program_imm = imm;
        trace.push(row);
    }

    trace.resize(trace.len().next_power_of_two(), ProgramColumnsView {
        // Some columns need special treatment.
        program_is_inst: F::ZERO,
        // The remaining columns duplicate their last value.
        ..*trace.last().unwrap()
    });

    trace
}

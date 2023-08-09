use std::collections::HashSet;

use mozak_vm::elf::Program;
use plonky2::hash::hash_types::RichField;

use crate::cpu::columns::{CpuState, Instruction};
use crate::program::columns::{InstColumnsView, ProgramColumnsView};
use crate::utils::pad_trace_with_default;

/// Generates a program ROM trace
#[must_use]
#[allow(clippy::missing_panics_doc)]
pub fn generate_program_rom_trace<F: RichField>(
    program: &Program,
    cpu_trace: &[CpuState<F>],
) -> Vec<ProgramColumnsView<F>> {
    let used_pcs: HashSet<F> = cpu_trace.iter().map(|row| row.inst.pc).collect();

    pad_trace_with_default(
        program
            .code
            .iter()
            .map(|(&pc, &inst)| ProgramColumnsView {
                filter: F::from_bool(used_pcs.contains(&F::from_canonical_u32(pc))),
                inst: InstColumnsView::from(
                    Instruction::from((pc, inst)).map(F::from_canonical_u32),
                ),
            })
            .collect(),
    )
}

use std::collections::HashSet;

use mozak_vm::elf::Code;
use plonky2::hash::hash_types::RichField;

use crate::cpu::columns::{CpuColumnsView, InstructionView};
use crate::program::columns::{InstColumnsView, ProgramColumnsView};

/// Generates a program trace from CPU traces.
///
/// Note: The ideal source for generating the program trace should be ELF file
/// instructions instead of CPU traces. This approach would require a
/// substantial refactoring, including the separation of local opcode decoding
/// from CPU trace generation.
#[must_use]
#[allow(clippy::missing_panics_doc)]
pub fn generate_program_trace<F: RichField>(
    code: &Code,
    cpu_trace: &[CpuColumnsView<F>],
) -> Vec<ProgramColumnsView<F>> {
    // NOTE: We expect CpuColumnsView to already be padded to the right size.
    let used_pcs: HashSet<F> = cpu_trace.iter().map(|row| row.inst.pc).collect();

    code.iter()
        .map(|(&pc, &inst)| ProgramColumnsView {
            filter: F::from_bool(used_pcs.contains(&F::from_canonical_u32(pc))),
            inst: InstColumnsView::from(
                InstructionView::from((pc, inst)).map(F::from_canonical_u32),
            ),
        })
        .collect()
}

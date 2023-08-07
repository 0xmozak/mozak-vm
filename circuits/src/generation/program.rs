use mozak_vm::elf::Program;
use plonky2::hash::hash_types::RichField;

use crate::cpu::columns::InstructionView;
use crate::program::columns::{InstColumnsView, ProgramColumnsView};
use crate::utils::pad_trace_with_default;

/// Generates a program ROM trace
#[must_use]
#[allow(clippy::missing_panics_doc)]
pub fn generate_program_rom_trace<F: RichField>(program: &Program) -> Vec<ProgramColumnsView<F>> {
    pad_trace_with_default(
        program
            .code
            .iter()
            .map(|(&pc, &inst)| ProgramColumnsView {
                filter: F::ONE,
                inst: InstColumnsView::from(
                    InstructionView::from((pc, inst)).map(F::from_canonical_u32),
                ),
            })
            .collect(),
    )
}

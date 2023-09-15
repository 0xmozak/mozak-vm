use mozak_executor::elf::Program;
use plonky2::hash::hash_types::RichField;

use crate::cpu::columns::Instruction;
use crate::program::columns::{InstructionRow, ProgramRom};
use crate::utils::pad_trace_with_default;

/// Generates a program ROM trace
#[must_use]
pub fn generate_program_rom_trace<F: RichField>(program: &Program) -> Vec<ProgramRom<F>> {
    pad_trace_with_default(
        program
            .ro_code
            .iter()
            .map(|(&pc, &inst)| ProgramRom {
                filter: F::ONE,
                inst: InstructionRow::from(
                    Instruction::from((pc, inst)).map(F::from_canonical_u32),
                ),
            })
            .collect(),
    )
}

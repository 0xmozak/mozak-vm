use mozak_runner::elf::Program;
use plonky2::hash::hash_types::RichField;

use crate::cpu::columns::Instruction;
use crate::program::columns::ProgramRom;
use crate::utils::pad_trace_with_last;

/// Generates a program ROM trace from a given program.
#[must_use]
pub fn generate_program_rom_trace<F: RichField>(program: &Program) -> Vec<ProgramRom<F>> {
    let mut roms = program
        .ro_code
        .iter()
        .filter_map(|(&pc, &inst)| {
            Some(ProgramRom::from(
                Instruction::from((pc, inst.ok()?)).map(F::from_canonical_u32),
            ))
        })
        .collect::<Vec<_>>();

    roms.sort_by_key(|entry| entry.pc.to_canonical_u64());

    pad_trace_with_last(roms)
}

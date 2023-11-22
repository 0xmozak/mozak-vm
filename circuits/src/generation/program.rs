use mozak_runner::elf::Program;
use mozak_runner::vm::ExecutionRecord;
use plonky2::hash::hash_types::RichField;

use crate::cpu::columns::Instruction;
use crate::program::columns::{InstructionRow, ProgramRom, RomMultiplicity};
use crate::utils::pad_trace_with_default;

/// Generates a program ROM trace from a given program.
#[must_use]
pub fn generate_program_rom_trace<F: RichField>(program: &Program) -> Vec<ProgramRom<F>> {
    let mut roms = program
        .ro_code
        .iter()
        .filter_map(|(&pc, &inst)| {
            Some(ProgramRom {
                filter: F::ONE,
                inst: InstructionRow::from(
                    Instruction::from((pc, inst.ok()?)).map(F::from_canonical_u32),
                ),
            })
        })
        .collect::<Vec<_>>();

    roms.sort_by_key(|entry| entry.inst.pc.to_canonical_u64());

    pad_trace_with_default(roms)
}

#[must_use]
pub fn generate_multiplicities<F: RichField>(
    record: &ExecutionRecord<F>,
    program_rom: &Vec<ProgramRom<F>>,
) -> Vec<RomMultiplicity<F>> {
    let mut multiplicities = vec![0; program_rom.len()];

    for row in &record.executed {
        multiplicities[row.state.pc as usize] += 1;
    }
    multiplicities
        .into_iter()
        .map(|m| RomMultiplicity {
            multiplicity: F::from_canonical_u32(m),
        })
        .collect()
}

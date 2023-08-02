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
    let used_pcs: HashSet<F> = cpu_trace.iter().map(|row| row.inst.pc).collect();

    code.iter()
        .map(|(&pc, &inst)| {
            let i: InstructionView<F> =
                InstructionView::from((pc, inst)).map(F::from_canonical_u32);
            let pc = &F::from_canonical_u32(pc);
            ProgramColumnsView {
                filter: F::from_bool(used_pcs.contains(pc)),
                inst: InstColumnsView::from(i),
            }
        })
        .collect()

    // let trace_len = unique_instructions.len().next_power_of_two();
    // let mut trace_res: [Vec<F>; NUM_PROGRAM_COLS] = Default::default();

    // for vec in &mut trace_res {
    //     vec.resize(trace_len, F::ZERO);
    // }
}

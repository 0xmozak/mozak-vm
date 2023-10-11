use itertools::{self, Itertools};
use mozak_runner::elf::Program;
use mozak_runner::instruction::Op;
use mozak_runner::vm::Row;
use plonky2::hash::hash_types::RichField;

use crate::memory::trace::get_memory_inst_clk;
use crate::memory_halfword::columns::{HalfWordMemory, Ops};

/// Pad the memory trace to a power of 2.
#[must_use]
fn pad_mem_trace<F: RichField>(mut trace: Vec<HalfWordMemory<F>>) -> Vec<HalfWordMemory<F>> {
    trace.resize(trace.len().next_power_of_two(), HalfWordMemory {
        // Some columns need special treatment..
        ops: Ops::default(),
        // .. and all other columns just have their last value duplicated.
        ..trace.last().copied().unwrap_or_default()
    });
    trace
}

/// Filter the memory trace to only include halfword load and store
/// instructions.
pub fn filter_memory_trace<'a, F: RichField>(
    program: &'a Program,
    step_rows: &'a [Row<F>],
) -> impl Iterator<Item = &'a Row<F>> {
    step_rows.iter().filter(|row| {
        matches!(
            row.state.current_instruction(program).op,
            Op::LH | Op::LHU | Op::SH
        )
    })
}

#[must_use]
pub fn generate_halfword_memory_trace<F: RichField>(
    program: &Program,
    step_rows: &[Row<F>],
) -> Vec<HalfWordMemory<F>> {
    pad_mem_trace(
        filter_memory_trace(program, step_rows)
            .map(|s| {
                let op = s.state.current_instruction(program).op;
                let mem_addr0 = s.aux.mem.unwrap_or_default().addr;
                let mem_addr1 = mem_addr0.wrapping_add(1);
                HalfWordMemory {
                    clk: get_memory_inst_clk(s),
                    addrs: [
                        F::from_canonical_u32(mem_addr0),
                        F::from_canonical_u32(mem_addr1),
                    ],
                    ops: Ops {
                        is_store: F::from_bool(matches!(op, Op::SH)),
                        is_load: F::from_bool(matches!(op, Op::LH | Op::LHU)),
                    },
                    limbs: [
                        F::from_canonical_u32(s.aux.dst_val & 0xFF),
                        F::from_canonical_u32((s.aux.dst_val >> 8) & 0xFF),
                    ],
                }
            })
            .collect_vec(),
    )
}

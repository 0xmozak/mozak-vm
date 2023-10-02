use itertools::{self, Itertools};
use mozak_runner::elf::Program;
use mozak_runner::instruction::Op;
use mozak_runner::vm::Row;
use plonky2::hash::hash_types::RichField;

use crate::memory::trace::{get_memory_inst_addr, get_memory_inst_clk};
use crate::memory_halfword::columns::HalfWordMemory;

/// Pad the memory trace to a power of 2.
#[must_use]
fn pad_mem_trace<F: RichField>(mut trace: Vec<HalfWordMemory<F>>) -> Vec<HalfWordMemory<F>> {
    trace.resize(trace.len().next_power_of_two(), HalfWordMemory {
        // Some columns need special treatment..
        is_sh: F::ZERO,
        is_lhu: F::ZERO,
        // .. and all other columns just have their last value duplicated.
        ..trace.last().copied().unwrap_or_default()
    });
    trace
}

/// Returns the rows sorted in the order of the instruction address.
/// TODO(Roman): consider maybe using memory-generation loop once and not
/// multiple times (refactoring)
#[must_use]
pub fn filter_memory_trace<'a>(
    program: &'a Program,
    step_rows: &'a [Row],
) -> impl Iterator<Item = &'a Row> {
    step_rows
        .iter()
        .filter(|row| matches!(row.state.current_instruction(program).op, Op::LHU | Op::SH))
        .sorted_by_key(|row| (row.aux.mem_addr, row.state.clk))
}

#[must_use]
pub fn generate_halfword_memory_trace<F: RichField>(
    program: &Program,
    step_rows: &[Row],
) -> Vec<HalfWordMemory<F>> {
    // If the trace length is not a power of two, we need to extend the trace to the
    // next power of two. The additional elements are filled with the last row
    // of the trace.
    pad_mem_trace(
        filter_memory_trace(program, step_rows)
            .map(|s| {
                let inst = s.state.current_instruction(program);
                let mem_clk = get_memory_inst_clk(s);
                let mem_addr = get_memory_inst_addr(s);
                HalfWordMemory {
                    clk: mem_clk,
                    addr: mem_addr,
                    is_sh: F::from_bool(matches!(inst.op, Op::SH)),
                    is_lhu: F::from_bool(matches!(inst.op, Op::LHU)),
                    addr_limb1: F::from_canonical_u32(
                        s.aux.mem_addr.unwrap_or_default().wrapping_add(1),
                    ),
                    limb0: F::from_canonical_u32(s.aux.dst_val & 0xFF),
                    limb1: F::from_canonical_u32((s.aux.dst_val >> 8) & 0xFF),
                    ..Default::default()
                }
            })
            .collect_vec(),
    )
}

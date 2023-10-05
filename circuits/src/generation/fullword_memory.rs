use itertools::{self, Itertools};
use mozak_runner::elf::Program;
use mozak_runner::instruction::Op;
use mozak_runner::vm::Row;
use plonky2::hash::hash_types::RichField;

use crate::memory::trace::{get_memory_inst_addr, get_memory_inst_clk};
use crate::memory_fullword::columns::{FullWordMemory, Ops};

/// Pad the memory trace to a power of 2.
#[must_use]
fn pad_mem_trace<F: RichField>(mut trace: Vec<FullWordMemory<F>>) -> Vec<FullWordMemory<F>> {
    trace.resize(trace.len().next_power_of_two(), FullWordMemory {
        // Some columns need special treatment..
        ops: Ops::default(),
        // .. and all other columns just have their last value duplicated.
        ..trace.last().copied().unwrap_or_default()
    });
    trace
}

/// Returns the rows sorted in the order of the instruction address.
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
) -> Vec<FullWordMemory<F>> {
    pad_mem_trace(
        filter_memory_trace(program, step_rows)
            .map(|s| {
                let op = s.state.current_instruction(program).op;
                let mut mem_addr: [F; 4] = [F::ZERO; 4];
                mem_addr[0] = get_memory_inst_addr(s);
                for i in 1..4 {
                    mem_addr[i] = F::from_canonical_u32(u32::wrapping_add(
                        s.aux.mem_addr.unwrap_or_default(),
                        i as u32,
                    ));
                }
                FullWordMemory {
                    clk: get_memory_inst_clk(s),
                    addrs: mem_addr,
                    ops: Ops {
                        is_store: F::from_bool(matches!(op, Op::SH)),
                        is_load: F::from_bool(matches!(op, Op::LHU)),
                    },
                    limbs: [
                        F::from_canonical_u32(s.aux.dst_val & 0xFF),
                        F::from_canonical_u32((s.aux.dst_val >> 8) & 0xFF),
                        F::from_canonical_u32((s.aux.dst_val >> 16) & 0xFF),
                        F::from_canonical_u32((s.aux.dst_val >> 24) & 0xFF),
                    ],
                }
            })
            .collect_vec(),
    )
}

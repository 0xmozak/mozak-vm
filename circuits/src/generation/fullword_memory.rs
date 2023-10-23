use itertools::{self, Itertools};
use mozak_runner::elf::Program;
use mozak_runner::instruction::Op;
use mozak_runner::vm::Row;
use plonky2::hash::hash_types::RichField;

use crate::memory::trace::get_memory_inst_clk;
use crate::memory_fullword::columns::{FullWordMemory, Ops};

/// Pad the memory trace to a power of 2.
#[must_use]
fn pad_mem_trace<F: RichField>(mut trace: Vec<FullWordMemory<F>>) -> Vec<FullWordMemory<F>> {
    trace.resize(trace.len().next_power_of_two().max(4), FullWordMemory {
        // Some columns need special treatment..
        ops: Ops::default(),
        // .. and all other columns just have their last value duplicated.
        ..trace.last().copied().unwrap_or_default()
    });
    trace
}

/// Returns the rows with full word memory instructions.
pub fn filter_memory_trace<'a, F: RichField>(
    program: &'a Program,
    step_rows: &'a [Row<F>],
) -> impl Iterator<Item = &'a Row<F>> {
    step_rows
        .iter()
        .filter(|row| matches!(row.state.current_instruction(program).op, Op::LW | Op::SW))
}

#[must_use]
pub fn generate_fullword_memory_trace<F: RichField>(
    program: &Program,
    step_rows: &[Row<F>],
) -> Vec<FullWordMemory<F>> {
    pad_mem_trace(
        filter_memory_trace(program, step_rows)
            .map(|s| {
                let op = s.state.current_instruction(program).op;
                let base_addr = s.aux.mem.unwrap_or_default().addr;
                let addrs = (0..4)
                    .map(|i| F::from_canonical_u32(base_addr.wrapping_add(i)))
                    .collect_vec()
                    .try_into()
                    .unwrap();
                let limbs = s
                    .aux
                    .dst_val
                    .to_le_bytes()
                    .into_iter()
                    .map(F::from_canonical_u8)
                    .collect_vec()
                    .try_into()
                    .unwrap();
                FullWordMemory {
                    clk: get_memory_inst_clk(s),
                    addrs,
                    ops: Ops {
                        is_store: F::from_bool(matches!(op, Op::SW)),
                        is_load: F::from_bool(matches!(op, Op::LW)),
                    },
                    limbs,
                }
            })
            .collect_vec(),
    )
}

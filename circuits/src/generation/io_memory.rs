use itertools::{self, Itertools};
use mozak_runner::elf::{Data, Program};
use mozak_runner::state::IoOpcode;
use mozak_runner::vm::Row;
use plonky2::hash::hash_types::RichField;

use crate::memory::trace::get_memory_inst_clk;
use crate::memory_io::columns::{InputOutputMemory, Ops};

/// Pad the memory trace to a power of 2.
#[must_use]
fn pad_io_mem_trace<F: RichField>(
    mut trace: Vec<InputOutputMemory<F>>,
) -> Vec<InputOutputMemory<F>> {
    trace.resize(trace.len().next_power_of_two(), InputOutputMemory {
        // Some columns need special treatment..
        ops: Ops::default(),
        // .. and all other columns just have their last value duplicated.
        ..trace.last().copied().unwrap_or_default()
    });
    trace
}

/// Returns the rows with full word memory instructions.
pub fn filter(step_rows: &[Row]) -> impl Iterator<Item = &Row> {
    step_rows.iter().filter(|row| {
        matches!(
            row.aux.io.unwrap_or_default().op,
            IoOpcode::Load | IoOpcode::Store
        )
    })
}

#[must_use]
pub fn generate_io_memory_trace<F: RichField>(
    program: &Program,
    step_rows: &[Row],
) -> Vec<InputOutputMemory<F>> {
    pad_io_mem_trace(
        filter(step_rows)
            .map(|s| {
                let item = InputOutputMemory {
                    clk: get_memory_inst_clk(s),
                    addr: F::from_canonical_u32(s.aux.io.unwrap_or_default().addr),
                    size: F::from_canonical_u32(s.aux.io.unwrap_or_default().size),
                    value: F::from_canonical_u32(0),
                    ops: Ops {
                        is_io_store: F::from_bool(matches!(
                            s.aux.io.unwrap_or_default().op,
                            IoOpcode::Store
                        )),
                        is_io_load: F::from_bool(matches!(
                            s.aux.io.unwrap_or_default().op,
                            IoOpcode::Load
                        )),
                        is_memory_store: F::from_bool(false),
                        is_memory_load: F::from_bool(false),
                    },
                };
                let mut extended = vec![];
                extended.fill(item);
                for i in 0..s.aux.io.unwrap_or_default().size {
                    let local_address = s.aux.io.unwrap_or_default().addr.wrapping_add(i);
                    let local_size = s.aux.io.unwrap_or_default().size - i;
                    let local_value = program.rw_memory.get(&local_address).unwrap().clone();
                    extended.fill(InputOutputMemory {
                        clk: get_memory_inst_clk(s),
                        addr: F::from_canonical_u32(local_address),
                        size: F::from_canonical_u32(local_size),
                        value: F::from_canonical_u8(local_value),
                        ops: Ops {
                            is_io_store: F::ZERO,
                            is_io_load: F::ZERO,
                            is_memory_store: item.ops.is_io_load,
                            is_memory_load: item.ops.is_io_store,
                        },
                    });
                }
                extended
            })
            .collect_vec()
            .into_iter()
            .flatten()
            .collect::<Vec<InputOutputMemory<F>>>(),
    )
}

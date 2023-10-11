use itertools::{self, Itertools};
use mozak_runner::elf::Program;
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
        ..Default::default()
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
                let local_op = s.aux.io.unwrap_or_default().op;
                let mut extended = vec![];
                for i in 0..s.aux.io.unwrap_or_default().size {
                    let local_address = s.aux.io.unwrap_or_default().addr.wrapping_add(i);
                    let local_size = s.aux.io.unwrap_or_default().size - i;
                    let local_value = program.rw_memory.get(&local_address).unwrap();
                    extended.fill(InputOutputMemory {
                        clk: get_memory_inst_clk(s),
                        addr: F::from_canonical_u32(local_address),
                        size: F::from_canonical_u32(local_size),
                        value: F::from_canonical_u8(*local_value),
                        ops: Ops {
                            is_io_store: if i == 0 {
                                F::from_bool(matches!(local_op, IoOpcode::Store))
                            } else {
                                F::ZERO
                            },
                            is_io_load: if i == 0 {
                                F::from_bool(matches!(local_op, IoOpcode::Load))
                            } else {
                                F::ZERO
                            },
                            is_memory_store: if i == 0 {
                                F::ZERO
                            } else {
                                F::from_bool(matches!(local_op, IoOpcode::Store))
                            },
                            is_memory_load: if i == 0 {
                                F::ZERO
                            } else {
                                F::from_bool(matches!(local_op, IoOpcode::Load))
                            },
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

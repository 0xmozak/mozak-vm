use itertools::{self, Itertools};
use mozak_runner::elf::Program;
use mozak_runner::state::{IoEntry, IoOpcode};
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

/// Returns the rows with io memory instructions.
pub fn filter<F: RichField>(step_rows: &[Row<F>]) -> impl Iterator<Item = &Row<F>> {
    step_rows.iter().filter(|row| {
        matches!(
            row.aux.io,
            Some(IoEntry {
                op: IoOpcode::Store,
                ..
            }),
        )
    })
}

#[must_use]
pub fn generate_io_memory_trace<F: RichField>(
    _program: &Program,
    step_rows: &[Row<F>],
) -> Vec<InputOutputMemory<F>> {
    pad_io_mem_trace(
        filter(step_rows)
            .map(|s| {
                let io = s.aux.io.clone().unwrap_or_default();
                let local_op = io.op;
                let mut extended = vec![];
                let value = io.data.first().copied().unwrap_or_default();
                // initial io-element
                extended.push(InputOutputMemory {
                    clk: get_memory_inst_clk(s),
                    addr: F::from_canonical_u32(io.addr),
                    size: F::from_canonical_u32(u32::try_from(io.data.len()).unwrap()),
                    value: F::from_canonical_u8(value),
                    ops: Ops {
                        is_io_store: F::from_bool(matches!(local_op, IoOpcode::Store)),
                        is_memory_store: F::ZERO,
                    },
                });
                // extended memory elements
                for (i, local_value) in io.data.iter().enumerate() {
                    let local_address = io.addr.wrapping_add(u32::try_from(i).unwrap());
                    let local_size = u32::try_from(io.data.len() - i - 1).unwrap();
                    extended.push(InputOutputMemory {
                        clk: get_memory_inst_clk(s),
                        addr: F::from_canonical_u32(local_address),
                        size: F::from_canonical_u32(local_size),
                        value: F::from_canonical_u8(*local_value),
                        ops: Ops {
                            is_io_store: F::ZERO,
                            is_memory_store: F::from_bool(matches!(local_op, IoOpcode::Store)),
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

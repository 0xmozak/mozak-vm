use itertools::{self, chain};
use mozak_runner::elf::Program;
use mozak_runner::instruction::Op;
use mozak_runner::state::{IoEntry, IoOpcode};
use mozak_runner::vm::Row;
use plonky2::hash::hash_types::RichField;

use crate::generation::MIN_TRACE_LENGTH;
use crate::memory::trace::get_memory_inst_clk;
use crate::memory_io::columns::{InputOutputMemory, Ops};

/// Pad the memory trace to a power of 2.
#[must_use]
fn pad_io_mem_trace<F: RichField>(
    mut trace: Vec<InputOutputMemory<F>>,
) -> Vec<InputOutputMemory<F>> {
    trace.resize(
        trace.len().max(MIN_TRACE_LENGTH).next_power_of_two(),
        InputOutputMemory::default(),
    );
    trace
}

/// Returns the rows with io memory instructions.
pub fn filter<'a, F: RichField>(
    program: &'a Program,
    step_rows: &'a [Row<F>],
) -> impl Iterator<Item = &'a Row<F>> {
    step_rows.iter().filter(|row| {
        matches!(
            row.aux.io,
            Some(IoEntry {
                op: IoOpcode::Store,
                ..
            }),
        ) && matches!(row.state.current_instruction(program).op, Op::ECALL,)
    })
}

#[must_use]
pub fn generate_io_memory_trace<F: RichField>(
    program: &Program,
    step_rows: &[Row<F>],
) -> Vec<InputOutputMemory<F>> {
    pad_io_mem_trace(
        filter(program, step_rows)
            .flat_map(|s| {
                let IoEntry { op, data, addr }: IoEntry = s.aux.io.clone().unwrap_or_default();
                let len = data.len();
                chain!(
                    // initial io-element
                    [InputOutputMemory {
                        clk: get_memory_inst_clk(s),
                        addr: F::from_canonical_u32(addr),
                        size: F::from_canonical_usize(len),
                        ops: Ops {
                            is_io_store: F::from_bool(matches!(op, IoOpcode::Store)),
                            is_memory_store: F::ZERO,
                        },
                        is_lv_and_nv_are_memory_rows: F::from_bool(false),
                        ..Default::default()
                    }],
                    // extended memory elements
                    data.into_iter().enumerate().map(move |(i, local_value)| {
                        let local_address = addr.wrapping_add(u32::try_from(i).unwrap());
                        let local_size = len - i - 1;
                        InputOutputMemory {
                            clk: get_memory_inst_clk(s),
                            addr: F::from_canonical_u32(local_address),
                            size: F::from_canonical_usize(local_size),
                            value: F::from_canonical_u8(local_value),
                            ops: Ops {
                                is_io_store: F::ZERO,
                                is_memory_store: F::from_bool(matches!(op, IoOpcode::Store)),
                            },
                            is_lv_and_nv_are_memory_rows: F::from_bool(i + 1 != len),
                        }
                    })
                )
            })
            .collect::<Vec<InputOutputMemory<F>>>(),
    )
}

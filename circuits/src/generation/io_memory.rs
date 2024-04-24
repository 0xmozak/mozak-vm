use itertools::chain;
use mozak_runner::instruction::Op;
use mozak_runner::state::{IoEntry, IoOpcode};
use mozak_runner::vm::Row;
use plonky2::hash::hash_types::RichField;

use crate::generation::MIN_TRACE_LENGTH;
use crate::memory::trace::get_memory_inst_clk;
use crate::memory_io::columns::{Ops, StorageDevice};

/// Pad the memory trace to a power of 2.
#[must_use]
fn pad_io_mem_trace<F: RichField>(mut trace: Vec<StorageDevice<F>>) -> Vec<StorageDevice<F>> {
    trace.resize(
        trace.len().max(MIN_TRACE_LENGTH).next_power_of_two(),
        StorageDevice::default(),
    );
    trace
}

/// Returns the rows with io memory instructions.
pub fn filter<F: RichField>(
    step_rows: &[Row<F>],
    which_tape: IoOpcode,
) -> impl Iterator<Item = &Row<F>> {
    step_rows.iter().filter(move |row| {
        (Some(which_tape) == row.aux.io.as_ref().map(|io| io.op))
            && matches!(row.instruction.op, Op::ECALL,)
    })
}
fn is_io_opcode<F: RichField>(op: IoOpcode) -> F {
    F::from_bool(matches!(
        op,
        IoOpcode::StorePrivate
            | IoOpcode::StorePublic
            | IoOpcode::StoreCallTape
            | IoOpcode::StoreEventsCommitmentTape
            | IoOpcode::StoreCastListCommitmentTape
    ))
}

#[must_use]
pub fn generate_io_memory_trace<F: RichField>(
    step_rows: &[Row<F>],
    which_tape: IoOpcode,
) -> Vec<StorageDevice<F>> {
    pad_io_mem_trace(
        filter(step_rows, which_tape)
            .flat_map(|s| {
                let IoEntry { op, data, addr }: IoEntry = s.aux.io.clone().unwrap_or_default();
                let len = data.len();
                chain!(
                    // initial io-element
                    [StorageDevice {
                        clk: get_memory_inst_clk(s),
                        addr: F::from_canonical_u32(addr),
                        size: F::from_canonical_usize(len),
                        ops: Ops {
                            is_io_store: is_io_opcode(op),
                            is_memory_store: F::ZERO,
                        },
                        is_lv_and_nv_are_memory_rows: F::from_bool(false),
                        ..Default::default()
                    }],
                    // extended memory elements
                    data.into_iter().enumerate().map(move |(i, local_value)| {
                        let local_address = addr.wrapping_add(u32::try_from(i).unwrap());
                        let local_size = len - i - 1;
                        StorageDevice {
                            clk: get_memory_inst_clk(s),
                            addr: F::from_canonical_u32(local_address),
                            size: F::from_canonical_usize(local_size),
                            value: F::from_canonical_u8(local_value),
                            ops: Ops {
                                is_io_store: F::ZERO,
                                is_memory_store: is_io_opcode(op),
                            },
                            is_lv_and_nv_are_memory_rows: F::from_bool(i + 1 != len),
                        }
                    })
                )
            })
            .collect::<Vec<StorageDevice<F>>>(),
    )
}

#[must_use]
pub fn generate_io_memory_private_trace<F: RichField>(
    step_rows: &[Row<F>],
) -> Vec<StorageDevice<F>> {
    generate_io_memory_trace(step_rows, IoOpcode::StorePrivate)
}

#[must_use]
pub fn generate_io_memory_public_trace<F: RichField>(
    step_rows: &[Row<F>],
) -> Vec<StorageDevice<F>> {
    generate_io_memory_trace(step_rows, IoOpcode::StorePublic)
}

#[must_use]
pub fn generate_call_tape_trace<F: RichField>(step_rows: &[Row<F>]) -> Vec<StorageDevice<F>> {
    generate_io_memory_trace(step_rows, IoOpcode::StoreCallTape)
}

#[must_use]
pub fn generate_events_commitment_tape_trace<F: RichField>(
    step_rows: &[Row<F>],
) -> Vec<StorageDevice<F>> {
    generate_io_memory_trace(step_rows, IoOpcode::StoreEventsCommitmentTape)
}

#[must_use]
pub fn generate_cast_list_commitment_tape_trace<F: RichField>(
    step_rows: &[Row<F>],
) -> Vec<StorageDevice<F>> {
    generate_io_memory_trace(step_rows, IoOpcode::StoreCastListCommitmentTape)
}

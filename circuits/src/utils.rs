use plonky2::field::types::Field;

use crate::generation::MIN_TRACE_LENGTH;

/// Pad the trace with a given `Row` to a power of 2.
///
/// # Panics
/// There's an assert that makes sure all columns passed in have the same
/// length.
#[must_use]
pub fn pad_trace_with_row<Row: Default + Clone>(mut trace: Vec<Row>, row: Row) -> Vec<Row> {
    let len = trace.len().next_power_of_two().max(MIN_TRACE_LENGTH);
    trace.resize(len, row);
    trace
}

/// Pad the trace with the trace's last `Row` to a power of 2.
#[must_use]
pub fn pad_trace_with_last<Row: Default + Clone>(mut trace: Vec<Row>) -> Vec<Row> {
    let len = trace.len().next_power_of_two().max(MIN_TRACE_LENGTH);
    trace.resize(len, trace.last().unwrap().clone());
    trace
}

#[must_use]
pub fn pad_trace_with_last_to_len<Row: Default + Clone>(
    mut trace: Vec<Row>,
    len: usize,
) -> Vec<Row> {
    trace.resize(len, trace.last().unwrap().clone());
    trace
}

#[must_use]
pub fn pad_trace_with_default_to_len<Row: Default + Clone>(
    mut trace: Vec<Row>,
    len: usize,
) -> Vec<Row> {
    trace.resize(len, Row::default());
    trace
}

/// Pad each row to the nearest power of two with the `Row`'s `Default`
/// implementation.
#[must_use]
pub fn pad_trace_with_default<Row: Default + Clone>(trace: Vec<Row>) -> Vec<Row> {
    let len = trace.len().next_power_of_two().max(MIN_TRACE_LENGTH);
    pad_trace_with_default_to_len(trace, len)
}

#[must_use]
pub(crate) fn from_u32<F: Field>(x: u32) -> F { Field::from_canonical_u32(x) }

#[must_use]
#[allow(clippy::cast_possible_wrap)]
pub fn sign_extend(is_signed: bool, x: u32) -> i64 {
    if is_signed {
        i64::from(x as i32)
    } else {
        i64::from(x)
    }
}

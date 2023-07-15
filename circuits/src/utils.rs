use itertools::Itertools;
use plonky2::field::types::Field;

/// Pad the trace to a power of 2.
///
/// # Panics
/// There's an assert that makes sure all columns passed in have the same
/// length.
#[must_use]
pub fn pad_trace<F: Field>(mut trace: Vec<Vec<F>>, clk_col: Option<usize>) -> Vec<Vec<F>> {
    assert!(trace
        .iter()
        .tuple_windows()
        .all(|(a, b)| a.len() == b.len()));
    trace.iter_mut().enumerate().for_each(|(i, col)| {
        if let (Some(padded_len), Some(&last)) = (col.len().checked_next_power_of_two(), col.last())
        {
            let extra = padded_len - col.len();
            if clk_col == Some(i) {
                col.extend(
                    (1_u64..)
                        .take(extra)
                        .map(|j| last + F::from_noncanonical_u64(j)),
                );
            } else {
                col.extend(vec![last; extra]);
            }
        }
    });
    trace
}

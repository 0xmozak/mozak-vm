use itertools::Itertools;
use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;

// TODO(Matthias): We can convert via u64, once https://github.com/mir-protocol/plonky2/pull/1092 has landed.
pub fn from_<X, F: Field>(x: X) -> F
where
    u128: From<X>, {
    Field::from_noncanonical_u128(u128::from(x))
}

#[must_use]
pub fn column_of_xs<P: PackedField>(x: u64) -> P { from_::<u64, P::Scalar>(x) * P::ONES }

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
                col.extend((1_u64..).take(extra).map(|j| last + from_(j)));
            } else {
                col.extend(vec![last; extra]);
            }
        }
    });
    trace
}

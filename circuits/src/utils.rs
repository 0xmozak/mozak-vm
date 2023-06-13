use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;

// TODO(Matthias): We can convert via u64, once https://github.com/mir-protocol/plonky2/pull/1092 has landed.
pub fn from_<X, F: Field>(x: X) -> F
where
    u128: From<X>,
{
    Field::from_noncanonical_u128(u128::from(x))
}

pub fn column_of_xs<X, P: PackedField>(x: X) -> P
where
    u128: From<X>,
{
    from_::<X, P::Scalar>(x) * P::ONES
}

/// Pad the trace to a power of 2.
#[must_use]
pub fn pad_trace<F: Field>(mut trace: Vec<Vec<F>>, clk_col: usize) -> Vec<Vec<F>> {
    trace.iter_mut().enumerate().for_each(|(i, col)| {
        if let (Some(padded_len), Some(&last)) = (col.len().checked_next_power_of_two(), col.last())
        {
            let extra = padded_len - col.len();
            if clk_col == i {
                col.extend((0_u64..).take(extra).map(|i| last + from_(i)));
            } else {
                col.extend(vec![last; extra]);
            }
        }
    });
    trace
}

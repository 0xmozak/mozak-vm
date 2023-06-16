use itertools::Itertools;
use mozak_vm::state::State;
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

pub fn pair_windows<S, Item>(it: S) -> impl Iterator<Item = (Item, Option<Item>)>
where
    S: Sized + Iterator<Item = Item>,
    Item: Clone,
{
    it.map(|x| Some(x))
        .chain(std::iter::once(None))
        .tuple_windows()
        .filter_map(|(a, b)| a.map(|a| (a, b)))
}

/// Convenience function to pair `State`s together (usually the i-th state and
/// the i+1-th state) for trace generation.
///
/// This returns the current state, and the value contained in rd in the next
/// state.
pub fn augment_dst<'a>(
    states: impl Iterator<Item = &'a State>,
) -> impl Iterator<Item = (&'a State, u32)> {
    pair_windows(states).map(move |(state, next_state)| {
        let dst = state.current_instruction().data.rd;
        let dst_val = next_state.map_or(0, |ns| ns.get_register_value(usize::from(dst)));
        (state, dst_val)
    })
}

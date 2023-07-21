use std::mem::{size_of, transmute_copy, ManuallyDrop};

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
                    (1..)
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

#[must_use]
pub(crate) fn from_u32<F: Field>(x: u32) -> F { Field::from_noncanonical_u64(x.into()) }

// TODO: rename
pub(crate) unsafe fn transmute_without_compile_time_size_checks<T, U>(t: T) -> U {
    debug_assert_eq!(size_of::<T>(), size_of::<U>());
    // We need to avoid `t` being dropped automatically, so we use ManuallyDrop.
    // We copy the bit pattern.  The original `t` is no longer safe to use,
    // (and that's why we pass it by move, not by reference).
    transmute_copy(&ManuallyDrop::<T>::new(t))
}

pub(crate) const fn indices_arr<const N: usize>() -> [usize; N] { core::array::from_fn(|i| i) }

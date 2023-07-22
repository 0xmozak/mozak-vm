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

// TODO(Matthias): sort out const'ness and replace with:
// pub(crate) const fn indices_arr<const N: usize>() -> [usize; N] {
// core::array::from_fn(|i| i) }
pub(crate) const fn indices_arr<const N: usize>() -> [usize; N] {
    let mut indices_arr = [0; N];
    let mut i = 0;
    while i < N {
        indices_arr[i] = i;
        i += 1;
    }
    indices_arr
}

// TODO(Matthias): this could probably be a custom derive macro?
macro_rules! boilerplate_implementations {
    ($s: ident, $num: ident) => {
        // `u8` is guaranteed to have a `size_of` of 1.
        pub(crate) const $num: usize = size_of::<$s<u8>>();

        impl<F: Field> Default for $s<F> {
            fn default() -> Self { Self::from([F::ZERO; $num]) }
        }

        impl<T: Copy> From<[T; $num]> for $s<T> {
            fn from(value: [T; $num]) -> Self {
                unsafe { transmute_without_compile_time_size_checks(value) }
            }
        }

        impl<T: Copy> From<$s<T>> for [T; $num] {
            fn from(value: $s<T>) -> Self {
                unsafe { transmute_without_compile_time_size_checks(value) }
            }
        }

        impl<T: Copy> Borrow<$s<T>> for [T; $num] {
            fn borrow(&self) -> &$s<T> { unsafe { &*(self as *const [T; $num]).cast::<$s<T>>() } }
        }

        impl<T: Copy> BorrowMut<$s<T>> for [T; $num] {
            fn borrow_mut(&mut self) -> &mut $s<T> {
                unsafe { &mut *(self as *mut [T; $num]).cast::<$s<T>>() }
            }
        }

        impl<T: Copy> Borrow<[T; $num]> for $s<T> {
            fn borrow(&self) -> &[T; $num] {
                unsafe { &*(self as *const $s<T>).cast::<[T; $num]>() }
            }
        }

        impl<T: Copy> BorrowMut<[T; $num]> for $s<T> {
            fn borrow_mut(&mut self) -> &mut [T; $num] {
                unsafe { &mut *(self as *mut $s<T>).cast::<[T; $num]>() }
            }
        }

        impl<T: Copy, I> Index<I> for $s<T>
        where
            [T]: Index<I>,
        {
            type Output = <[T] as Index<I>>::Output;

            fn index(&self, index: I) -> &Self::Output {
                let arr: &[T; $num] = self.borrow();
                <[T] as Index<I>>::index(arr, index)
            }
        }

        impl<T: Copy, I> IndexMut<I> for $s<T>
        where
            [T]: IndexMut<I>,
        {
            fn index_mut(&mut self, index: I) -> &mut Self::Output {
                let arr: &mut [T; $num] = self.borrow_mut();
                <[T] as IndexMut<I>>::index_mut(arr, index)
            }
        }
    };
}
pub(crate) use boilerplate_implementations;

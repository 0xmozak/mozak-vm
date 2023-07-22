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

pub(crate) unsafe fn transmute_without_compile_time_size_checks<T, U>(t: T) -> U {
    debug_assert_eq!(size_of::<T>(), size_of::<U>());
    // We need to avoid `t` being dropped automatically, so we use ManuallyDrop.
    // We copy the bit pattern.  The original `t` is no longer safe to use,
    // (and that's why we pass it by move, not by reference).
    transmute_copy(&ManuallyDrop::<T>::new(t))
}

// TODO(Matthias): sort out const'ness and replace with:
pub(crate) fn indices_arr<const N: usize>() -> [usize; N] { core::array::from_fn(|i| i) }

pub trait NumberOfColumns {
    const NUMBER_OF_COLUMNS: usize;
}

// TODO(Matthias): this could probably be a custom derive macro?
macro_rules! boilerplate_implementations {
    ($s: ident) => {
        impl<T: Copy> crate::utils::NumberOfColumns for $s<T> {
            // `u8` is guaranteed to have a `size_of` of 1.
            const NUMBER_OF_COLUMNS: usize = std::mem::size_of::<$s<u8>>();
        }

        impl<F: Field> Default for $s<F> {
            fn default() -> Self { Self::from([F::ZERO; std::mem::size_of::<$s<u8>>()]) }
        }

        impl<T: Copy> From<[T; std::mem::size_of::<$s<u8>>()]> for $s<T> {
            fn from(value: [T; std::mem::size_of::<$s<u8>>()]) -> Self {
                unsafe { transmute_without_compile_time_size_checks(value) }
            }
        }

        impl<T: Copy> From<$s<T>> for [T; std::mem::size_of::<$s<u8>>()] {
            fn from(value: $s<T>) -> Self {
                unsafe { transmute_without_compile_time_size_checks(value) }
            }
        }

        impl<T: Copy> std::borrow::Borrow<$s<T>> for [T; std::mem::size_of::<$s<u8>>()] {
            fn borrow(&self) -> &$s<T> {
                unsafe { &*(self as *const [T; std::mem::size_of::<$s<u8>>()]).cast::<$s<T>>() }
            }
        }

        impl<T: Copy> std::borrow::BorrowMut<$s<T>> for [T; std::mem::size_of::<$s<u8>>()] {
            fn borrow_mut(&mut self) -> &mut $s<T> {
                unsafe { &mut *(self as *mut [T; std::mem::size_of::<$s<u8>>()]).cast::<$s<T>>() }
            }
        }

        impl<T: Copy> std::borrow::Borrow<[T; std::mem::size_of::<$s<u8>>()]> for $s<T> {
            fn borrow(&self) -> &[T; std::mem::size_of::<$s<u8>>()] {
                unsafe { &*(self as *const $s<T>).cast::<[T; std::mem::size_of::<$s<u8>>()]>() }
            }
        }

        impl<T: Copy> std::borrow::BorrowMut<[T; std::mem::size_of::<$s<u8>>()]> for $s<T> {
            fn borrow_mut(&mut self) -> &mut [T; std::mem::size_of::<$s<u8>>()] {
                unsafe { &mut *(self as *mut $s<T>).cast::<[T; std::mem::size_of::<$s<u8>>()]>() }
            }
        }

        impl<T: Copy, I> std::ops::Index<I> for $s<T>
        where
            [T]: std::ops::Index<I>,
        {
            type Output = <[T] as std::ops::Index<I>>::Output;

            fn index(&self, index: I) -> &Self::Output {
                use std::borrow::Borrow;
                let arr: &[T; std::mem::size_of::<$s<u8>>()] = self.borrow();
                <[T] as std::ops::Index<I>>::index(arr, index)
            }
        }

        impl<T: Copy, I> std::ops::IndexMut<I> for $s<T>
        where
            [T]: std::ops::IndexMut<I>,
        {
            fn index_mut(&mut self, index: I) -> &mut Self::Output {
                use std::borrow::BorrowMut;
                let arr: &mut [T; std::mem::size_of::<$s<u8>>()] = self.borrow_mut();
                <[T] as std::ops::IndexMut<I>>::index_mut(arr, index)
            }
        }
    };
}
pub(crate) use boilerplate_implementations;

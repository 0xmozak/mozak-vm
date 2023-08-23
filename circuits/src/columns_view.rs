//! This module makes STARK table row values indexing simpler by providing
//! an abstraction for column-by-name access instead of direct number indexing.
//! This is achieved by using the macros below.
//!
//! This way, they can be nested to group columns by logic they handle.

use std::mem::{size_of, transmute_copy, ManuallyDrop};
use std::ops::IndexMut;

pub(crate) unsafe fn transmute_without_compile_time_size_checks<T, U>(t: T) -> U {
    debug_assert_eq!(size_of::<T>(), size_of::<U>());
    // We need to avoid `t` being dropped automatically, so we use ManuallyDrop.
    // We copy the bit pattern.  The original `t` is no longer safe to use,
    // (and that's why we pass it by move, not by reference).
    transmute_copy(&ManuallyDrop::<T>::new(t))
}

pub trait NumberOfColumns {
    const NUMBER_OF_COLUMNS: usize;
}

// TODO(Matthias): this could probably be a custom derive macro?
/// Functions to handle and seamlessly convert between `SubTableView` with named
/// fields and default `[T, ColumnSize]` column representations.
///
/// ### Conceptual Example
///
/// Now, instead of accessing columns by `columns[i]` one can instead access
/// them as `new_columns_repr.filter_column` and at the same time `columns` can
/// `new_columns_repr` can be seamlessly converted between each other.
macro_rules! columns_view_impl {
    ($s: ident) => {
        impl<T> $s<T> {
            // At the moment we only use `map` Instruction,
            // so it's dead code for the other callers of `columns_view_impl`.
            // TODO(Matthias): remove this marker, once we use it for the other structs,
            // too.
            #[allow(dead_code)]
            pub fn map<B: std::fmt::Debug, F>(self, f: F) -> $s<B>
            where
                F: FnMut(T) -> B, {
                self.into_iter().map(f).collect()
            }
        }

        impl<T> crate::columns_view::NumberOfColumns for $s<T> {
            // `u8` is guaranteed to have a `size_of` of 1.
            const NUMBER_OF_COLUMNS: usize = std::mem::size_of::<$s<u8>>();
        }

        impl<T> From<[T; std::mem::size_of::<$s<u8>>()]> for $s<T> {
            fn from(value: [T; std::mem::size_of::<$s<u8>>()]) -> Self {
                unsafe { crate::columns_view::transmute_without_compile_time_size_checks(value) }
            }
        }

        impl<T> From<$s<T>> for [T; std::mem::size_of::<$s<u8>>()] {
            fn from(value: $s<T>) -> Self {
                unsafe { crate::columns_view::transmute_without_compile_time_size_checks(value) }
            }
        }

        impl<T> std::borrow::Borrow<$s<T>> for [T; std::mem::size_of::<$s<u8>>()] {
            fn borrow(&self) -> &$s<T> {
                unsafe { &*(self as *const [T; std::mem::size_of::<$s<u8>>()]).cast::<$s<T>>() }
            }
        }

        impl<T> std::borrow::BorrowMut<$s<T>> for [T; std::mem::size_of::<$s<u8>>()] {
            fn borrow_mut(&mut self) -> &mut $s<T> {
                unsafe { &mut *(self as *mut [T; std::mem::size_of::<$s<u8>>()]).cast::<$s<T>>() }
            }
        }

        impl<T> std::borrow::Borrow<[T; std::mem::size_of::<$s<u8>>()]> for $s<T> {
            fn borrow(&self) -> &[T; std::mem::size_of::<$s<u8>>()] {
                unsafe { &*(self as *const $s<T>).cast::<[T; std::mem::size_of::<$s<u8>>()]>() }
            }
        }

        impl<T> std::borrow::BorrowMut<[T; std::mem::size_of::<$s<u8>>()]> for $s<T> {
            fn borrow_mut(&mut self) -> &mut [T; std::mem::size_of::<$s<u8>>()] {
                unsafe { &mut *(self as *mut $s<T>).cast::<[T; std::mem::size_of::<$s<u8>>()]>() }
            }
        }

        impl<T, I> std::ops::Index<I> for $s<T>
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

        impl<T, I> std::ops::IndexMut<I> for $s<T>
        where
            [T]: std::ops::IndexMut<I>,
        {
            fn index_mut(&mut self, index: I) -> &mut Self::Output {
                use std::borrow::BorrowMut;
                let arr: &mut [T; std::mem::size_of::<$s<u8>>()] = self.borrow_mut();
                <[T] as std::ops::IndexMut<I>>::index_mut(arr, index)
            }
        }

        impl<T> std::iter::IntoIterator for $s<T> {
            type IntoIter = std::array::IntoIter<T, { std::mem::size_of::<$s<u8>>() }>;
            type Item = T;

            fn into_iter(self) -> Self::IntoIter {
                let array: [T; std::mem::size_of::<$s<u8>>()] = self.into();
                array.into_iter()
            }
        }

        impl<'a, T> std::iter::IntoIterator for &'a $s<T> {
            type IntoIter = std::slice::Iter<'a, T>;
            type Item = &'a T;

            fn into_iter(self) -> Self::IntoIter {
                use std::borrow::Borrow;
                let array: &[T; std::mem::size_of::<$s<u8>>()] = self.borrow();
                array.into_iter()
            }
        }

        impl<T: std::fmt::Debug> std::iter::FromIterator<T> for $s<T> {
            fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
                let vec: Vec<T> = iter.into_iter().collect();
                let array: [T; std::mem::size_of::<$s<u8>>()] = vec.try_into().unwrap();
                array.into()
            }
        }
    };
}
pub(crate) use columns_view_impl;

macro_rules! make_col_map {
    ($s: ident) => {
        lazy_static::lazy_static! {
            // TODO(Matthias): sort out const'ness of from_fn, and declare as a const instead of static:
            pub(crate) static ref MAP: $s<usize> = {
                use crate::columns_view::NumberOfColumns;
                const COLUMNS: usize = $s::<()>::NUMBER_OF_COLUMNS;
                let indices_arr: [usize; COLUMNS] = core::array::from_fn(|i| i);
                unsafe { std::mem::transmute::<[usize; COLUMNS], $s<usize>>(indices_arr) }
            };
        }
    };
}
pub(crate) use make_col_map;

#[must_use]
pub fn selection<T: IndexMut<usize, Output = u32> + Default>(which: usize) -> T {
    let mut selectors = T::default();
    selectors[which] = 1;
    selectors
}

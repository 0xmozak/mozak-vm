//! This module makes STARK table row values indexing simpler by providing
//! an abstraction for column-by-name access instead of direct number indexing.
//! This is achieved by using the macros below.
//!
//! This way, they can be nested to group columns by logic they handle.

use std::marker::PhantomData;
use std::mem::{size_of, ManuallyDrop};

pub(crate) const unsafe fn transmute_without_compile_time_size_checks<T, U>(t: T) -> U {
    #[repr(C)]
    union MyUnion<T, U> {
        t: ManuallyDrop<T>,
        u: ManuallyDrop<U>,
    }

    debug_assert!(size_of::<T>() == size_of::<U>());

    // We need to avoid `t` being dropped automatically, so we use ManuallyDrop.
    // We copy the bit pattern.  The original `t` is no longer safe to use,
    // (and that's why we pass it by move, not by reference).
    let t = ManuallyDrop::new(t);
    ManuallyDrop::into_inner(MyUnion { t }.u)
}
pub(crate) const unsafe fn transmute_ref<T, U>(t: &T) -> &U {
    debug_assert!(size_of::<T>() == size_of::<U>());
    &*((t as *const T).cast::<U>())
}

pub trait HasNamedColumns {
    type Columns;
}

pub trait NumberOfColumns {
    const NUMBER_OF_COLUMNS: usize;
}

/// This structure only exists to improve macro impl hiding
#[doc(hidden)]
pub struct ColumnViewImplHider<T>(PhantomData<T>);

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
        // This hides all the `unsafe` from clippy
        impl<T> crate::columns_view::ColumnViewImplHider<$s<T>> {
            const fn from_array(value: [T; std::mem::size_of::<$s<u8>>()]) -> $s<T> {
                unsafe { crate::columns_view::transmute_without_compile_time_size_checks(value) }
            }

            const fn into_array(v: $s<T>) -> [T; std::mem::size_of::<$s<u8>>()] {
                unsafe { crate::columns_view::transmute_without_compile_time_size_checks(v) }
            }

            const fn from_array_ref(value: &[T; std::mem::size_of::<$s<u8>>()]) -> &$s<T> {
                unsafe { crate::columns_view::transmute_ref(value) }
            }

            const fn array_ref(v: &$s<T>) -> &[T; std::mem::size_of::<$s<u8>>()] {
                unsafe { crate::columns_view::transmute_ref(v) }
            }
        }

        impl<T> $s<T> {
            const fn from_array(value: [T; std::mem::size_of::<$s<u8>>()]) -> Self {
                crate::columns_view::ColumnViewImplHider::<Self>::from_array(value)
            }

            const fn into_array(self) -> [T; std::mem::size_of::<$s<u8>>()] {
                crate::columns_view::ColumnViewImplHider::<Self>::into_array(self)
            }

            const fn from_array_ref(value: &[T; std::mem::size_of::<$s<u8>>()]) -> &Self {
                crate::columns_view::ColumnViewImplHider::<Self>::from_array_ref(value)
            }

            const fn array_ref(&self) -> &[T; std::mem::size_of::<$s<u8>>()] {
                crate::columns_view::ColumnViewImplHider::<Self>::array_ref(self)
            }

            pub fn iter(&self) -> std::slice::Iter<T> { self.array_ref().into_iter() }

            // At the moment we only use `map` Instruction,
            // so it's dead code for the other callers of `columns_view_impl`.
            // TODO(Matthias): remove this marker, once we use it for the other structs,
            // too.
            #[allow(dead_code)]
            pub fn map<B, F>(self, f: F) -> $s<B>
            where
                F: FnMut(T) -> B, {
                $s::from_array(self.into_array().map(f))
            }
        }

        impl<T> crate::columns_view::NumberOfColumns for $s<T> {
            // `u8` is guaranteed to have a `size_of` of 1.
            const NUMBER_OF_COLUMNS: usize = std::mem::size_of::<$s<u8>>();
        }

        impl<T> From<[T; std::mem::size_of::<$s<u8>>()]> for $s<T> {
            fn from(value: [T; std::mem::size_of::<$s<u8>>()]) -> Self { Self::from_array(value) }
        }

        impl<T> From<$s<T>> for [T; std::mem::size_of::<$s<u8>>()] {
            fn from(value: $s<T>) -> Self { value.into_array() }
        }

        impl<'a, T> From<&'a [T]> for &'a $s<T> {
            fn from(value: &'a [T]) -> Self {
                let value: &[T; std::mem::size_of::<$s<u8>>()] =
                    value.try_into().expect("slice of correct length");
                $s::from_array_ref(value)
            }
        }

        impl<T> std::borrow::Borrow<[T]> for $s<T> {
            fn borrow(&self) -> &[T] { self.array_ref() }
        }

        impl<T, I> std::ops::Index<I> for $s<T>
        where
            [T]: std::ops::Index<I>,
        {
            type Output = <[T] as std::ops::Index<I>>::Output;

            fn index(&self, index: I) -> &Self::Output { &self.array_ref()[index] }
        }

        impl<T> std::iter::IntoIterator for $s<T> {
            type IntoIter = std::array::IntoIter<T, { std::mem::size_of::<$s<u8>>() }>;
            type Item = T;

            fn into_iter(self) -> Self::IntoIter { self.into_array().into_iter() }
        }

        impl<'a, T> std::iter::IntoIterator for &'a $s<T> {
            type IntoIter = std::slice::Iter<'a, T>;
            type Item = &'a T;

            fn into_iter(self) -> Self::IntoIter { self.iter() }
        }

        impl<T: std::fmt::Debug> std::iter::FromIterator<T> for $s<T> {
            fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
                const LEN: usize = std::mem::size_of::<$s<u8>>();
                let vec: arrayvec::ArrayVec<T, LEN> = iter.into_iter().collect();
                let array = vec.into_inner().expect("iterator of correct length");
                Self::from_array(array)
            }
        }
    };
}

pub(crate) use columns_view_impl;

#[must_use]
pub const fn col_map<const NUMBER_OF_COLUMNS: usize>() -> [usize; NUMBER_OF_COLUMNS] {
    let mut indices_arr = [0usize; NUMBER_OF_COLUMNS];
    let mut i = 0;
    while i < indices_arr.len() {
        indices_arr[i] = i;
        i += 1;
    }
    indices_arr
}

/// Implement a static `MAP` of the `ColumnsView` from an array with length
/// [`NumberOfColumns`] of the `ColumnsView` that allows for indexing into an
/// array with the column name rather than the column index.
macro_rules! make_col_map {
    ($s: ident) => {
        pub(crate) const fn col_map() -> &'static $s<usize> {
            const MAP: $s<usize> = {
                use crate::columns_view::NumberOfColumns;
                const NUMBER_OF_COLUMNS: usize = $s::<()>::NUMBER_OF_COLUMNS;
                $s::from_array(crate::columns_view::col_map::<NUMBER_OF_COLUMNS>())
            };
            &MAP
        }
    };
}
pub(crate) use make_col_map;

/// Return a selector that is only active at index `which`
#[must_use]
pub fn selection<T, const NUMBER_OF_COLUMNS: usize>(which: usize) -> T
where
    T: From<[u32; NUMBER_OF_COLUMNS]>, {
    let mut indices_arr = [0; NUMBER_OF_COLUMNS];
    indices_arr[which] = 1;
    indices_arr.into()
}

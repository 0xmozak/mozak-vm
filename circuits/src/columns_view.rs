use std::mem::{size_of, transmute_copy, ManuallyDrop};

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
macro_rules! columns_view_impl {
    ($s: ident) => {
        impl<T: Copy> crate::columns_view::NumberOfColumns for $s<T> {
            // `u8` is guaranteed to have a `size_of` of 1.
            const NUMBER_OF_COLUMNS: usize = std::mem::size_of::<$s<u8>>();
        }

        impl<T: Copy> From<[T; std::mem::size_of::<$s<u8>>()]> for $s<T> {
            fn from(value: [T; std::mem::size_of::<$s<u8>>()]) -> Self {
                unsafe { crate::columns_view::transmute_without_compile_time_size_checks(value) }
            }
        }

        impl<T: Copy> From<$s<T>> for [T; std::mem::size_of::<$s<u8>>()] {
            fn from(value: $s<T>) -> Self {
                unsafe { crate::columns_view::transmute_without_compile_time_size_checks(value) }
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

        impl<T: Copy> std::iter::IntoIterator for $s<T> {
            type IntoIter = std::array::IntoIter<T, { std::mem::size_of::<$s<u8>>() }>;
            type Item = T;

            fn into_iter(self) -> Self::IntoIter {
                let array: [T; std::mem::size_of::<$s<u8>>()] = self.into();
                array.into_iter()
            }
        }

        impl<T: plonky2::field::types::Field> std::iter::FromIterator<T> for $s<T> {
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

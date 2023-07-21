use std::borrow::{Borrow, BorrowMut};
use std::mem::{size_of, transmute};
use std::ops::{Index, IndexMut, RangeInclusive};

use plonky2::field::types::Field;

use crate::utils::{indices_arr, transmute_without_compile_time_size_checks};

#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub(crate) struct BitwiseColumnsView<T: Copy> {
    pub(crate) execution: BitwiseExecutionColumnsView<T>,

    // TODO(Matthias): separate out the permutation columns etc into a suitable separate structs,
    // too.
    pub(crate) op1_limbs_permuted: [T; 4],
    pub(crate) op2_limbs_permuted: [T; 4],
    pub(crate) res_limbs_permuted: [T; 4],

    // Each row holds result of compression of OP1_LIMB, OP2_LIMB and RES_LIMBS.
    pub(crate) compress_limbs: [T; 4],
    pub(crate) compress_permuted: [T; 4],

    pub(crate) fix_range_check_u8: T,
    pub(crate) fix_range_check_u8_permuted: [T; 12],

    pub(crate) fix_bitwise_op1: T,
    pub(crate) fix_bitwise_op2: T,
    pub(crate) fix_bitwise_res: T,

    pub(crate) fix_compress: T,
    pub(crate) fix_compress_permuted: [T; 4],
}

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

boilerplate_implementations!(BitwiseColumnsView, NUM_BITWISE_COL);

const fn make_col_map() -> BitwiseColumnsView<usize> {
    let indices_arr = indices_arr::<NUM_BITWISE_COL>();
    unsafe { transmute::<[usize; NUM_BITWISE_COL], BitwiseColumnsView<usize>>(indices_arr) }
}

pub(crate) const COL_MAP: BitwiseColumnsView<usize> = make_col_map();

#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub(crate) struct BitwiseExecutionColumnsView<T: Copy> {
    pub(crate) op1: T,
    pub(crate) op2: T,
    pub(crate) res: T,

    pub(crate) op1_limbs: [T; 4],
    pub(crate) op2_limbs: [T; 4],
    pub(crate) res_limbs: [T; 4],
}

boilerplate_implementations!(BitwiseExecutionColumnsView, NUM_BITWISE_TRACE_COL);

pub(crate) const RANGE_U8: RangeInclusive<u8> = u8::MIN..=u8::MAX; // 256 different values
pub(crate) const BITWISE_U8_SIZE: usize = 1 << 16; // 256 * 256 different possible combinations
pub(crate) const BASE: u16 = 256;

use std::borrow::{Borrow, BorrowMut};
use std::mem::{size_of, transmute};
use std::ops::{Index, IndexMut, RangeInclusive};

use plonky2::field::types::Field;

use crate::utils::{indices_arr, transmute_without_compile_time_size_checks};

#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub(crate) struct BitwiseColumnsView<T: Copy> {
    pub(crate) OP1: T,
    pub(crate) OP2: T,
    pub(crate) RES: T,

    pub(crate) OP1_LIMBS: [T; 4],
    pub(crate) OP2_LIMBS: [T; 4],
    pub(crate) RES_LIMBS: [T; 4],

    pub(crate) OP1_LIMBS_PERMUTED: [T; 4],
    pub(crate) OP2_LIMBS_PERMUTED: [T; 4],
    pub(crate) RES_LIMBS_PERMUTED: [T; 4],

    // Each row holds result of compression of OP1_LIMB, OP2_LIMB and RES_LIMBS.
    pub(crate) COMPRESS_LIMBS: [T; 4],
    pub(crate) COMPRESS_PERMUTED: [T; 4],

    pub(crate) FIX_RANGE_CHECK_U8: T,
    pub(crate) FIX_RANGE_CHECK_U8_PERMUTED: [T; 12],

    pub(crate) FIX_BITWISE_OP1: T,
    pub(crate) FIX_BITWISE_OP2: T,
    pub(crate) FIX_BITWISE_RES: T,

    pub(crate) FIX_COMPRESS: T,
    pub(crate) FIX_COMPRESS_PERMUTED: [T; 4],
}

// `u8` is guaranteed to have a `size_of` of 1.
pub(crate) const NUM_BITWISE_COL: usize = size_of::<BitwiseColumnsView<u8>>();

// TODO(Matthias): we could probably make a derive-macro for this?
impl<F: Field> Default for BitwiseColumnsView<F> {
    fn default() -> Self { Self::from([F::ZERO; NUM_BITWISE_COL]) }
}

impl<T: Copy> From<[T; NUM_BITWISE_COL]> for BitwiseColumnsView<T> {
    fn from(value: [T; NUM_BITWISE_COL]) -> Self {
        unsafe { transmute_without_compile_time_size_checks(value) }
    }
}

impl<T: Copy> From<BitwiseColumnsView<T>> for [T; NUM_BITWISE_COL] {
    fn from(value: BitwiseColumnsView<T>) -> Self {
        unsafe { transmute_without_compile_time_size_checks(value) }
    }
}

impl<T: Copy> Borrow<BitwiseColumnsView<T>> for [T; NUM_BITWISE_COL] {
    fn borrow(&self) -> &BitwiseColumnsView<T> { unsafe { transmute(self) } }
}

impl<T: Copy> BorrowMut<BitwiseColumnsView<T>> for [T; NUM_BITWISE_COL] {
    fn borrow_mut(&mut self) -> &mut BitwiseColumnsView<T> { unsafe { transmute(self) } }
}

impl<T: Copy> Borrow<[T; NUM_BITWISE_COL]> for BitwiseColumnsView<T> {
    fn borrow(&self) -> &[T; NUM_BITWISE_COL] { unsafe { transmute(self) } }
}

impl<T: Copy> BorrowMut<[T; NUM_BITWISE_COL]> for BitwiseColumnsView<T> {
    fn borrow_mut(&mut self) -> &mut [T; NUM_BITWISE_COL] { unsafe { transmute(self) } }
}

impl<T: Copy, I> Index<I> for BitwiseColumnsView<T>
where
    [T]: Index<I>,
{
    type Output = <[T] as Index<I>>::Output;

    fn index(&self, index: I) -> &Self::Output {
        let arr: &[T; NUM_BITWISE_COL] = self.borrow();
        <[T] as Index<I>>::index(arr, index)
    }
}

impl<T: Copy, I> IndexMut<I> for BitwiseColumnsView<T>
where
    [T]: IndexMut<I>,
{
    fn index_mut(&mut self, index: I) -> &mut Self::Output {
        let arr: &mut [T; NUM_BITWISE_COL] = self.borrow_mut();
        <[T] as IndexMut<I>>::index_mut(arr, index)
    }
}

const fn make_col_map() -> BitwiseColumnsView<usize> {
    let indices_arr = indices_arr::<NUM_BITWISE_COL>();
    unsafe { transmute::<[usize; NUM_BITWISE_COL], BitwiseColumnsView<usize>>(indices_arr) }
}

pub const COL_MAP: BitwiseColumnsView<usize> = make_col_map();

// ---
pub(crate) const RANGE_U8: RangeInclusive<u8> = u8::MIN..=u8::MAX; // 256 different values
pub(crate) const BITWISE_U8_SIZE: usize = 1 << 16; // 256 * 256 different possible combinations
pub(crate) const BASE: u16 = 256;

// --- Move to circuit utils:

use std::mem::transmute;
use std::ops::RangeInclusive;

use lazy_static::lazy_static;
use plonky2::field::types::Field;

use crate::utils::{
    boilerplate_implementations, make_col_map, transmute_without_compile_time_size_checks,
    NumberOfColumns,
};

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
boilerplate_implementations!(BitwiseColumnsView);
make_col_map!(BitwiseColumnsView);

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
boilerplate_implementations!(BitwiseExecutionColumnsView);

pub(crate) const RANGE_U8: RangeInclusive<u8> = u8::MIN..=u8::MAX; // 256 different values
pub(crate) const BITWISE_U8_SIZE: usize = 1 << 16; // 256 * 256 different possible combinations
pub(crate) const BASE: u16 = 256;

use std::ops::RangeInclusive;

use crate::columns_view::{columns_view_impl, make_col_map};

#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub(crate) struct BitwiseColumnsView<T: Copy> {
    pub(crate) execution: BitwiseExecutionColumnsView<T>,

    // TODO(Matthias): separate out the permutation columns etc into suitable separate structs,
    // too.
    pub(crate) op1_limbs_permuted: [T; 4],
    pub(crate) op2_limbs_permuted: [T; 4],
    pub(crate) res_limbs_permuted: [T; 4],

    // Each row holds result of compression of OP1_LIMB, OP2_LIMB and RES_LIMBS.
    pub(crate) compressed_limbs: [T; 4],
    pub(crate) compressed_permuted: [T; 4],

    pub(crate) fixed_range_check_u8: T,
    pub(crate) fixed_range_check_u8_permuted: [T; 12],

    pub(crate) fixed_bitwise_op1: T,
    pub(crate) fixed_bitwise_op2: T,
    pub(crate) fixed_bitwise_res: T,

    pub(crate) fixed_compressed: T,
    pub(crate) fixed_compressed_permuted: [T; 4],
}
columns_view_impl!(BitwiseColumnsView);
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
columns_view_impl!(BitwiseExecutionColumnsView);

pub(crate) const RANGE_U8: RangeInclusive<u8> = u8::MIN..=u8::MAX; // 256 different values
pub(crate) const BITWISE_U8_SIZE: usize = 1 << 16; // 256 * 256 different possible combinations
pub(crate) const BASE: u16 = 256;

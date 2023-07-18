use std::ops::{Range, RangeInclusive};

pub(crate) const OP1: usize = 0;
pub(crate) const OP2: usize = OP1 + 1;
pub(crate) const RES: usize = OP2 + 1;

pub(crate) const OP1_LIMBS: Range<usize> = RES + 1..RES + 5; // 7
pub(crate) const OP2_LIMBS: Range<usize> = OP1_LIMBS.end..OP1_LIMBS.end + 4; // 11
pub(crate) const RES_LIMBS: Range<usize> = OP2_LIMBS.end..OP2_LIMBS.end + 4; // 15

pub(crate) const OP1_LIMBS_PERMUTED: Range<usize> = RES_LIMBS.end..RES_LIMBS.end + 4; // 19
pub(crate) const OP2_LIMBS_PERMUTED: Range<usize> =
    OP1_LIMBS_PERMUTED.end..OP1_LIMBS_PERMUTED.end + 4; // 23
pub(crate) const RES_LIMBS_PERMUTED: Range<usize> =
    OP2_LIMBS_PERMUTED.end..OP2_LIMBS_PERMUTED.end + 4; // 27

// Each row holds result of compression of OP1_LIMB, OP2_LIMB and RES_LIMBS.
pub(crate) const COMPRESS_LIMBS: Range<usize> = RES_LIMBS_PERMUTED.end..RES_LIMBS_PERMUTED.end + 4; // 31
pub(crate) const COMPRESS_PERMUTED: Range<usize> = COMPRESS_LIMBS.end..COMPRESS_LIMBS.end + 4; // 35

pub(crate) const FIX_RANGE_CHECK_U8: usize = COMPRESS_PERMUTED.end; // 36
pub(crate) const FIX_RANGE_CHECK_U8_PERMUTED: Range<usize> =
    FIX_RANGE_CHECK_U8 + 1..FIX_RANGE_CHECK_U8 + 13; // 48

pub(crate) const FIX_BITWISE_OP1: usize = FIX_RANGE_CHECK_U8_PERMUTED.end; // 49
pub(crate) const FIX_BITWISE_OP2: usize = FIX_BITWISE_OP1 + 1; // 50
pub(crate) const FIX_BITWISE_RES: usize = FIX_BITWISE_OP2 + 1; // 51

pub(crate) const FIX_COMPRESS: usize = FIX_BITWISE_RES + 1; // 52
pub(crate) const FIX_COMPRESS_PERMUTED: Range<usize> = FIX_COMPRESS + 1..FIX_COMPRESS + 5; // 56

pub(crate) const NUM_BITWISE_COL: usize = FIX_COMPRESS_PERMUTED.end;

pub(crate) const RANGE_U8: RangeInclusive<u8> = u8::MIN..=u8::MAX; // 256 different values
pub(crate) const BITWISE_U8_SIZE: usize = 1 << 16; // 256 * 256 different possible combinations
pub(crate) const BASE: u16 = 256;

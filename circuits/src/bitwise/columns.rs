use std::ops::Range;

pub(crate) const OP1: usize = 0;
pub(crate) const OP2: usize = OP1 + 1;
pub(crate) const RES: usize = OP2 + 1;

pub(crate) const OP1_LIMBS: Range<usize> = RES + 1..RES + 5;
pub(crate) const OP2_LIMBS: Range<usize> = OP1_LIMBS.end..OP1_LIMBS.end + 4;
pub(crate) const RES_LIMBS: Range<usize> = OP2_LIMBS.end..OP2_LIMBS.end + 4;

pub(crate) const OP1_LIMBS_PERMUTED: Range<usize> = RES_LIMBS.end..RES_LIMBS.end + 4;
pub(crate) const OP2_LIMBS_PERMUTED: Range<usize> =
    OP1_LIMBS_PERMUTED.end..OP1_LIMBS_PERMUTED.end + 4;
pub(crate) const RES_LIMBS_PERMUTED: Range<usize> =
    OP2_LIMBS_PERMUTED.end..OP2_LIMBS_PERMUTED.end + 4;

pub(crate) const FIX_RANGE_CHECK_U8: usize = RES_LIMBS_PERMUTED.end;
pub(crate) const FIX_RANGE_CHECK_U8_PERMUTED: Range<usize> =
    FIX_RANGE_CHECK_U8 + 1..FIX_RANGE_CHECK_U8 + 5 + 8;

pub(crate) const FIX_BITWISE_OP1: usize = FIX_RANGE_CHECK_U8_PERMUTED.end;
pub(crate) const FIX_BITWISE_OP2: usize = FIX_BITWISE_OP1 + 1;
pub(crate) const FIX_BITWISE_RES: usize = FIX_BITWISE_OP2 + 1;

pub(crate) const NUM_BITWISE_COL: usize = FIX_BITWISE_RES + 1;

pub(crate) const RANGE_CHECK_U8_SIZE: usize = 1 << 8; // 256 different values
pub(crate) const BITWISE_U8_SIZE: usize = 1 << 16; // 256 * 256 different possible combinations

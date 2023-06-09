pub(crate) const VAL: usize = 0;
pub(crate) const LIMB_LO: usize = VAL + 1;
pub(crate) const LIMB_HI: usize = LIMB_LO + 1;
pub(crate) const LIMB_LO_PERMUTED: usize = LIMB_HI + 1;
pub(crate) const LIMB_HI_PERMUTED: usize = LIMB_LO_PERMUTED + 1;

pub(crate) const FIX_RANGE_CHECK_U16: usize = LIMB_HI_PERMUTED + 1;
pub(crate) const FIX_RANGE_CHECK_U16_PERMUTED_LO: usize = FIX_RANGE_CHECK_U16 + 1;
pub(crate) const FIX_RANGE_CHECK_U16_PERMUTED_HI: usize = FIX_RANGE_CHECK_U16_PERMUTED_LO + 1;

pub(crate) const COL_NUM_RC: usize = FIX_RANGE_CHECK_U16_PERMUTED_HI + 1; // 11

pub(crate) const RANGE_CHECK_U16_SIZE: usize = 1 << 16; // 4

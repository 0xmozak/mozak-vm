/// Column containing the value (in u32) to be range checked.
pub(crate) const VAL: usize = 0;

/// Column containing the lower limb (u16) of the u32 value to be range checked.
pub(crate) const LIMB_LO: usize = VAL + 1;

/// Column containing the upper limb (u16) of the u32 value to be range checked.
pub(crate) const LIMB_HI: usize = LIMB_LO + 1;

/// Permuted column containing the lower limb (u16) of the u32 value to be range
/// checked.
pub(crate) const LIMB_LO_PERMUTED: usize = LIMB_HI + 1;

/// Permuted column containing the upper limb (u16) of the u32 value to be range
/// checked.
pub(crate) const LIMB_HI_PERMUTED: usize = LIMB_LO_PERMUTED + 1;

/// Column to indicate that a value to be range checked is from the CPU table.
pub(crate) const CPU_FILTER: usize = LIMB_HI_PERMUTED + 1;

/// Fixed column containing values 0, 1, .., 2^16 - 1.
pub(crate) const FIXED_RANGE_CHECK_U16: usize = CPU_FILTER + 1;

/// Fixed column containing values 0, 1, .., 2^16 - 1. This is used in the
/// fixed table lookup argument for the lower 16-bit limb.
pub(crate) const FIXED_RANGE_CHECK_U16_PERMUTED_LO: usize = FIXED_RANGE_CHECK_U16 + 1;

/// Fixed column containing values 0, 1, .., 2^16 - 1. This is used in the
/// fixed table lookup argument for the upper 16-bit limb.
pub(crate) const FIXED_RANGE_CHECK_U16_PERMUTED_HI: usize = FIXED_RANGE_CHECK_U16_PERMUTED_LO + 1;

/// Total number of columns for the range check table.
pub(crate) const NUM_RC_COLS: usize = FIXED_RANGE_CHECK_U16_PERMUTED_HI + 1;

// Columns native to the range check table.
pub(crate) const VAL: usize = 0;
pub(crate) const LIMB_LO: usize = VAL + 1;
pub(crate) const LIMB_HI: usize = LIMB_LO + 1;
pub(crate) const LIMB_LO_PERMUTED: usize = LIMB_HI + 1;
pub(crate) const LIMB_HI_PERMUTED: usize = LIMB_LO_PERMUTED + 1;

// Columns for filters, used in cross table lookups.
pub(crate) const CPU_FILTER: usize = LIMB_HI_PERMUTED + 1;

// Columns for fixed u16 range check tables.
pub(crate) const FIXED_RANGE_CHECK_U16: usize = CPU_FILTER + 1;
pub(crate) const FIXED_RANGE_CHECK_U16_PERMUTED_LO: usize = FIXED_RANGE_CHECK_U16 + 1;
pub(crate) const FIXED_RANGE_CHECK_U16_PERMUTED_HI: usize = FIXED_RANGE_CHECK_U16_PERMUTED_LO + 1;

//
pub(crate) const NUM_RC_COLS: usize = FIXED_RANGE_CHECK_U16_PERMUTED_HI + 1; // 11

pub(crate) const RANGE_CHECK_U16_SIZE: usize = 1 << 16; // 4

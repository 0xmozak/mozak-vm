use itertools::Itertools;
use plonky2::field::types::Field;

use crate::cross_table_lookup::Column;

/// Column containing the value (in u32) to be range checked.
pub(crate) const VALUE: usize = 0;

/// Column containing the lower limb (u16) of the u32 value to be range checked.
pub(crate) const LIMB_LO: usize = VALUE + 1;

/// Column containing the upper limb (u16) of the u32 value to be range checked.
pub(crate) const LIMB_HI: usize = LIMB_LO + 1;

/// Permuted column containing the lower limb (u16) of the u32 value to be range
/// checked.
pub(crate) const LIMB_LO_PERMUTED: usize = LIMB_HI + 1;

/// Permuted column containing the upper limb (u16) of the u32 value to be range
/// checked.
pub(crate) const LIMB_HI_PERMUTED: usize = LIMB_LO_PERMUTED + 1;

/// Column to rangecheck CPU ADD instruction.
pub(crate) const S_DST_VALUE: usize = LIMB_HI_PERMUTED + 1;

/// Column to rangecheck CPU ADD instruction.
pub(crate) const S_OP1_VAL_FIXED: usize = S_DST_VALUE + 1;

/// Column to rangecheck CPU ADD instruction.
pub(crate) const S_OP2_VAL_FIXED: usize = S_OP1_VAL_FIXED + 1;

/// Column to rangecheck CPU ADD instruction.
pub(crate) const S_CMP_ABS_DIFF: usize = S_OP2_VAL_FIXED + 1;

/// Fixed column containing values 0, 1, .., 2^16 - 1.
pub(crate) const FIXED_RANGE_CHECK_U16: usize = S_CMP_ABS_DIFF + 1;

/// Fixed column containing values 0, 1, .., 2^16 - 1. This is used in the
/// fixed table lookup argument for the lower 16-bit limb.
pub(crate) const FIXED_RANGE_CHECK_U16_PERMUTED_LO: usize = FIXED_RANGE_CHECK_U16 + 1;

/// Fixed column containing values 0, 1, .., 2^16 - 1. This is used in the
/// fixed table lookup argument for the upper 16-bit limb.
pub(crate) const FIXED_RANGE_CHECK_U16_PERMUTED_HI: usize = FIXED_RANGE_CHECK_U16_PERMUTED_LO + 1;

/// Total number of columns for the range check table.
pub(crate) const NUM_RC_COLS: usize = FIXED_RANGE_CHECK_U16_PERMUTED_HI + 1;

/// Columns containing the data to be range checked in the Mozak
/// [`RangeCheckTable`](crate::cross_table_lookup::RangeCheckTable).
#[must_use]
pub fn data_for_cpu<F: Field>() -> Vec<Column<F>> { Column::singles([VALUE]).collect_vec() }

#[must_use]
pub fn filter_cpu_op1_val_fixed<F: Field>() -> Column<F> { Column::single(S_OP1_VAL_FIXED) }

#[must_use]
pub fn filter_cpu_op2_val_fixed<F: Field>() -> Column<F> { Column::single(S_OP2_VAL_FIXED) }

#[must_use]
pub fn filter_cpu_cmp_abs_diff<F: Field>() -> Column<F> { Column::single(S_CMP_ABS_DIFF) }

/// Column for a binary filter to indicate a range check from the
/// [`CpuTable`](crate::cross_table_lookup::CpuTable) in the Mozak
/// [`RangeCheckTable`](crate::cross_table_lookup::RangeCheckTable).
#[must_use]
pub fn filter_cpu_dst_value<F: Field>() -> Column<F> { Column::single(S_DST_VALUE) }

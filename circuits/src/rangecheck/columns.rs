use itertools::Itertools;
use plonky2::field::types::Field;

use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};
use crate::cross_table_lookup::Column;

#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub(crate) struct RangeCheckColumnsView<T: Copy> {
    /// Column containing the value (in u32) to be range checked.
    pub(crate) val: T,

    /// Column containing the lower limb (u16) of the u32 value to be range
    /// checked.
    pub(crate) limb_lo: T,

    /// Column containing the upper limb (u16) of the u32 value to be range
    /// checked.
    pub(crate) limb_hi: T,

    /// Permuted column containing the lower limb (u16) of the u32 value to be
    /// range checked.
    pub(crate) limb_lo_permuted: T,

    /// Permuted column containing the upper limb (u16) of the u32 value to be
    /// range checked.
    pub(crate) limb_hi_permuted: T,

    // Selector columns
    pub(crate) s_dst_value: T,
    pub(crate) s_op1_val_fixed: T,
    pub(crate) s_op2_val_fixed: T,
    pub(crate) s_cmp_abs_diff: T,

    /// Fixed column containing values 0, 1, .., 2^16 - 1.
    pub(crate) fixed_range_check_u16: T,

    /// Fixed column containing values 0, 1, .., 2^16 - 1. This is used in the
    /// fixed table lookup argument for the lower 16-bit limb.
    pub(crate) fixed_range_check_u16_permuted_lo: T,

    /// Fixed column containing values 0, 1, .., 2^16 - 1. This is used in the
    /// fixed table lookup argument for the upper 16-bit limb.
    pub(crate) fixed_range_check_u16_permuted_hi: T,
}
columns_view_impl!(RangeCheckColumnsView);
make_col_map!(RangeCheckColumnsView);

/// Total number of columns for the range check table.
pub(crate) const NUM_RC_COLS: usize = RangeCheckColumnsView::<()>::NUMBER_OF_COLUMNS;

/// Columns containing the data to be range checked in the Mozak
/// [`RangeCheckTable`](crate::cross_table_lookup::RangeCheckTable).
#[must_use]
pub fn data_for_cpu<F: Field>() -> Vec<Column<F>> { Column::singles([MAP.val]).collect_vec() }

#[must_use]
pub fn filter_cpu_op1_val_fixed<F: Field>() -> Column<F> { Column::single(MAP.s_op1_val_fixed) }

#[must_use]
pub fn filter_cpu_op2_val_fixed<F: Field>() -> Column<F> { Column::single(MAP.s_op2_val_fixed) }

#[must_use]
pub fn filter_cpu_cmp_abs_diff<F: Field>() -> Column<F> { Column::single(MAP.s_cmp_abs_diff) }

/// Column for a binary filter to indicate a range check from the
/// [`CpuTable`](crate::cross_table_lookup::CpuTable) in the Mozak
/// [`RangeCheckTable`](crate::cross_table_lookup::RangeCheckTable).
#[must_use]
pub fn filter_cpu_dst_value<F: Field>() -> Column<F> { Column::single(MAP.s_dst_value) }

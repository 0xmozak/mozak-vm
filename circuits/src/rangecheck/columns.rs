use plonky2::field::types::Field;

use crate::columns_view::{columns_view_impl, make_col_map};
use crate::cross_table_lookup::Column;

make_col_map!(RangeCheckColumnsExtended);
columns_view_impl!(RangeCheckColumnsExtended);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct RangeCheckColumnsExtended<T> {
    pub rangecheck: RangeCheckColumnsView<T>,
    pub permuted: InnerLookupColumnsView<T>,
}

columns_view_impl!(RangeCheckColumnsView);
/// View into the columns containing u32 values to be range checked from
/// other tables, along with their limbs and filter columns. This view
/// is involved with cross table lookups.
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct RangeCheckColumnsView<T> {
    /// Column containing the value (in u32) to be range checked.
    pub(crate) val: T,

    /// Column containing the lower limb (u16) of the u32 value to be range
    /// checked.
    pub(crate) limb_lo: T,

    /// Column containing the upper limb (u16) of the u32 value to be range
    /// checked.
    pub(crate) limb_hi: T,

    /// Column containing the upper limb (u16) of the u32 value to be range
    /// checked.
    pub(crate) cpu_filter: T,
}

columns_view_impl!(InnerLookupColumnsView);
/// View into the columns containing fixed columns and permuted limbs from
/// [`RangeCheckColumnsView`]. This view is involved with inner table lookups.
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct InnerLookupColumnsView<T> {
    /// Permuted column containing the lower limb (u16) of the u32 value to be
    /// range checked.
    pub(crate) limb_lo_permuted: T,

    /// Permuted column containing the upper limb (u16) of the u32 value to be
    /// range checked.
    pub(crate) limb_hi_permuted: T,

    /// Fixed column containing values 0, 1, .., 2^16 - 1. This is used in the
    /// fixed table lookup argument for the lower 16-bit limb.
    pub(crate) fixed_range_check_u16_permuted_lo: T,

    /// Fixed column containing values 0, 1, .., 2^16 - 1. This is used in the
    /// fixed table lookup argument for the upper 16-bit limb.
    pub(crate) fixed_range_check_u16_permuted_hi: T,

    /// Fixed column containing values 0, 1, .., 2^16 - 1.
    pub(crate) fixed_range_check_u16: T,
}

/// Columns containing the data to be range checked in the Mozak
/// [`RangeCheckTable`](crate::cross_table_lookup::RangeCheckTable).
#[must_use]
pub fn data_for_cpu<F: Field>() -> Vec<Column<F>> { vec![Column::single(MAP.rangecheck.val)] }

/// Column for a binary filter to indicate a range check from the
/// [`CpuTable`](crate::cross_table_lookup::CpuTable) in the Mozak
/// [`RangeCheckTable`](crate::cross_table_lookup::RangeCheckTable).
#[must_use]
pub fn filter_for_cpu<F: Field>() -> Column<F> { Column::single(MAP.rangecheck.cpu_filter) }

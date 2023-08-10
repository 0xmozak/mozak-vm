use plonky2::field::types::Field;

use crate::columns_view::{columns_view_impl, make_col_map};
use crate::cross_table_lookup::Column;

make_col_map!(RangeCheckColumnsView);
columns_view_impl!(RangeCheckColumnsView);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct RangeCheckColumnsView<T> {
    pub input: InputColumnsView<T>,
    pub permuted: U16InnerLookupColumnsView<T>,
}

columns_view_impl!(InputColumnsView);
/// View into the columns containing u32 values to be range checked from
/// other tables, along with their limbs and filter columns. This view
/// is involved with cross table lookups.
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct InputColumnsView<T> {
    /// Column containing the value (in u32) to be range checked.
    pub(crate) u32_value: T,

    /// Column containing the lower limb (u16) of the u32 value to be range
    /// checked.
    pub(crate) limb_lo: T,

    /// Column containing the upper limb (u16) of the u32 value to be range
    /// checked.
    pub(crate) limb_hi: T,

    /// Filter column to filter values from the CPU table.
    pub(crate) cpu_filter: T,
}

columns_view_impl!(U16InnerLookupColumnsView);
/// View into the columns containing fixed columns and permuted limbs from
/// [`RangeCheckColumnsView`]. This view is involved with inner table lookups.
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct U16InnerLookupColumnsView<T> {
    /// Permuted column containing the lower limb (u16) of the u32 value to be
    /// range checked.
    pub(crate) limb_lo_permuted: T,

    /// Permuted column containing the upper limb (u16) of the u32 value to be
    /// range checked.
    pub(crate) limb_hi_permuted: T,

    /// Fixed column containing values 0, 1, .., 2^16 - 1. This is used in the
    /// fixed table lookup argument for the lower 16-bit limb.
    pub(crate) fixed_range_permuted_lo: T,

    /// Fixed column containing values 0, 1, .., 2^16 - 1. This is used in the
    /// fixed table lookup argument for the upper 16-bit limb.
    pub(crate) fixed_range_permuted_hi: T,

    /// Fixed column containing values 0, 1, .., 2^16 - 1.
    pub(crate) fixed_range: T,
}

/// Columns containing the data to be range checked in the Mozak
/// [`RangeCheckTable`](crate::cross_table_lookup::RangeCheckTable).
#[must_use]
pub fn data_for_cpu<F: Field>() -> Vec<Column<F>> { vec![Column::single(MAP.input.u32_value)] }

/// Column for a binary filter to indicate a range check from the
/// [`CpuTable`](crate::cross_table_lookup::CpuTable) in the Mozak
/// [`RangeCheckTable`](crate::cross_table_lookup::RangeCheckTable).
#[must_use]
pub fn filter_for_cpu<F: Field>() -> Column<F> { Column::single(MAP.input.cpu_filter) }

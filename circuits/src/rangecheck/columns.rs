use plonky2::field::types::Field;

use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};
use crate::cross_table_lookup::Column;

#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub(crate) struct RangeCheckColumnsView<T> {
    /// Column containing the lower limb (u16) of the u32 value to be range
    /// checked.
    pub(crate) limb_lo: T,

    /// Column containing the upper limb (u16) of the u32 value to be range
    /// checked.
    pub(crate) limb_hi: T,

    /// Column to indicate that a value to be range checked is not a dummy
    /// value.
    pub(crate) filter: T,

    /// Fixed column containing values 0, 1, .., 2^16 - 1.
    pub(crate) multiplicities: T,

    /// Fixed column containing values 0, 1, .., 2^16 - 1.
    pub(crate) fixed_range_check_u16: T,
}
columns_view_impl!(RangeCheckColumnsView);
make_col_map!(RangeCheckColumnsView);

/// Total number of columns for the range check table.
pub(crate) const NUM_RC_COLS: usize = RangeCheckColumnsView::<()>::NUMBER_OF_COLUMNS;

/// Columns containing the data to be range checked in the Mozak
/// [`RangeCheckTable`](crate::cross_table_lookup::RangeCheckTable).
#[must_use]
pub fn data<F: Field>() -> Vec<Column<F>> {
    vec![Column::single(MAP.limb_lo) + Column::single(MAP.limb_hi) * F::from_canonical_u32(1 << 16)]
}

/// Column for a binary filter to indicate whether a row in the
/// [`RangeCheckTable`](crate::cross_table_lookup::RangeCheckTable).
/// contains a non-dummy value to be range checked.
#[must_use]
pub fn filter<F: Field>() -> Column<F> { Column::single(MAP.filter) }

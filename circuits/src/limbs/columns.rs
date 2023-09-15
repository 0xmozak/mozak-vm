use plonky2::field::types::Field;

use crate::columns_view::{columns_view_impl, make_col_map};
use crate::cross_table_lookup::Column;

#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct Limbs<T> {
    pub range_check_u16: T,
    // TODO: check whether we could get by without a filter?
    pub filter: T,
}
columns_view_impl!(Limbs);
make_col_map!(Limbs);

// /// Columns containing the data to be range checked in the Mozak
// /// [`RangeCheckTable`](crate::cross_table_lookup::RangeCheckTable).
// #[must_use]
// pub fn data_incoming<F: Field>() -> Vec<Column<F>> {
//     vec![Column::single(MAP.limb_lo) + Column::single(MAP.limb_hi) *
// F::from_canonical_u32(1 << 16)] }

// #[must_use]
// pub fn data_outgoing<F: Field>() -> Vec<Column<F>> {
//     vec![Column::single(MAP.limb_lo), Column::single(MAP.limb_hi)]
// }

#[must_use]
pub fn data<F: Field>() -> Vec<Column<F>> { vec![Column::single(MAP.range_check_u16)] }

/// Column for a binary filter to indicate whether a row in the
/// [`RangeCheckTable`](crate::cross_table_lookup::RangeCheckTable).
/// contains a non-dummy value to be range checked.
#[must_use]
pub fn filter<F: Field>() -> Column<F> { Column::single(MAP.filter) }

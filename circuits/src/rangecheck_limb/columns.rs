use plonky2::field::types::Field;

use crate::columns_view::{columns_view_impl, make_col_map};
use crate::cross_table_lookup::Column;
use crate::multiplicity_view::MultiplicityView;

#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct RangeCheckLimb<T> {
    /// The u8 value to be range checked and its multiplicity.
    pub multiplicity_view: MultiplicityView<T>,
}
columns_view_impl!(RangeCheckLimb);
make_col_map!(RangeCheckLimb);

#[must_use]
pub fn data<F: Field>() -> Vec<Column<F>> {
    vec![Column::single(col_map().multiplicity_view.value)]
}

/// Column for a binary filter to indicate whether a row in the
/// [`RangeCheckTable`](crate::cross_table_lookup::RangeCheckTable).
/// contains a non-dummy value to be range checked.
#[must_use]
pub fn filter<F: Field>() -> Column<F> { Column::single(col_map().multiplicity_view.multiplicity) }

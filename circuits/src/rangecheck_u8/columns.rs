use crate::columns_view::{columns_view_impl, make_col_map};
use crate::cross_table_lookup::Column;
use crate::stark::mozak_stark::{RangeCheckU8Table, Table};

#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct RangeCheckU8<T> {
    /// The u8 value to be range checked
    pub value: T,

    /// The frequencies for which the accompanying value occur in
    /// the trace. This is m(x) in the paper.
    pub multiplicity: T,
}
columns_view_impl!(RangeCheckU8);
make_col_map!(RangeCheckU8);

#[must_use]
pub fn lookup() -> Table {
    RangeCheckU8Table::new(
        vec![Column::single(col_map().value)],
        Column::single(col_map().multiplicity),
    )
}

#[must_use]
pub fn make_rows_public() -> Table {
    RangeCheckU8Table::new(vec![Column::single(col_map().value)], Column::constant(1))
}

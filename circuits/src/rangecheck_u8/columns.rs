use crate::columns_view::{columns_view_impl, make_col_map};
use crate::linear_combination_x::ColumnX;
use crate::rangecheck::columns::RangeCheckCtl;

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

type RangeCheckU8Col = ColumnX<RangeCheckU8<i64>>;

#[must_use]
pub fn data() -> RangeCheckCtl<RangeCheckU8Col> {
    RangeCheckCtl {
        value: COL_MAP.value,
    }
}

/// Column for a binary filter to indicate whether a row in the
/// [`RangeCheckTable`](crate::cross_table_lookup::RangeCheckTable).
/// contains a non-dummy value to be range checked.
#[must_use]
pub fn filter() -> RangeCheckU8Col { COL_MAP.multiplicity }

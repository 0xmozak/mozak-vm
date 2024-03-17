use crate::columns_view::{columns_view_impl, make_col_map};
use crate::linear_combination::Column;
use crate::rangecheck::columns::RangeCheckCtl;
use crate::stark::mozak_stark::{RangeCheckU8Table, TableNamed};

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
pub fn lookup() -> TableNamed<RangeCheckCtl<Column>> {
    RangeCheckU8Table::new(
        RangeCheckCtl {
            value: COL_MAP.value,
        },
        COL_MAP.multiplicity,
    )
}

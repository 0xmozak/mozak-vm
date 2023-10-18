use plonky2::field::types::Field;

use crate::columns_view::{columns_view_impl, make_col_map};
use crate::cross_table_lookup::Column;
use crate::stark::mozak_stark::{RangeCheckTable, Table};

columns_view_impl!(MultiplicityView);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct MultiplicityView<T> {
    /// The unique value.
    pub value: T,

    /// The frequencies for which the accompanying value occur in
    /// the trace. This is m(x) in the paper.
    pub multiplicity: T,
}

make_col_map!(RangeCheckColumnsView);
columns_view_impl!(RangeCheckColumnsView);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct RangeCheckColumnsView<T> {
    /// The u8 limbs.
    pub limbs: [T; 4],

    /// The filter.
    pub filter: T,

    /// logup
    pub logup_u32: MultiplicityView<T>,
}

/// Columns containing the data to be range checked in the Mozak
/// [`RangeCheckTable`](crate::cross_table_lookup::RangeCheckTable).
#[must_use]
pub fn data<F: Field>() -> Vec<Column<F>> { vec![Column::always()] }

/// Column for a binary filter to indicate whether a row in the
/// [`RangeCheckTable`](crate::cross_table_lookup::RangeCheckTable).
/// contains a non-dummy value to be range checked.
#[must_use]
pub fn filter<F: Field>() -> Column<F> { Column::always() }

#[must_use]
pub fn rangecheck_looking<F: Field>() -> Vec<Table<F>> {
    (0..4)
        .map(|limb| {
            RangeCheckTable::new(
                Column::singles([MAP.limbs[limb]]),
                Column::single(MAP.filter),
            )
        })
        .collect()
}

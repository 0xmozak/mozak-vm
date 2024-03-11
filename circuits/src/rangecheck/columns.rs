use plonky2::field::types::Field;

use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};
use crate::cross_table_lookup::Column;
use crate::stark::mozak_stark::{RangeCheckTable, TableNamed};

#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct RangeCheckColumnsView<T> {
    /// The limbs (u8) of the u32 value to be range
    /// checked.
    pub limbs: [T; 4],
    pub multiplicity: T,
}
columns_view_impl!(RangeCheckColumnsView);
make_col_map!(RangeCheckColumnsView);

/// Total number of columns for the range check table.
pub(crate) const NUM_RC_COLS: usize = RangeCheckColumnsView::<()>::NUMBER_OF_COLUMNS;

/// Columns containing the data to be range checked in the Mozak
/// [`RangeCheckTable`](crate::cross_table_lookup::RangeCheckTable).
#[must_use]
pub fn data_filter<F: Field>() -> TableNamed<F, RangeCheckCtl<Column<F>>> {
    let data = RangeCheckCtl::new(
        (0..4)
            .map(|limb| {
                Column::single(col_map().limbs[limb]) * F::from_canonical_u32(1 << (8 * limb))
            })
            .sum(),
    );
    RangeCheckTable::new(data, filter())
}

columns_view_impl!(RangeCheckCtl);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct RangeCheckCtl<T> {
    pub value: T,
}

impl<T> RangeCheckCtl<T> {
    pub fn new(value: T) -> Self { Self { value } }
}

#[must_use]
pub fn rangecheck_looking<F: Field>() -> Vec<TableNamed<F, RangeCheckCtl<Column<F>>>> {
    (0..4)
        .map(|limb| {
            RangeCheckTable::new(
                RangeCheckCtl::new(Column::from(col_map().limbs[limb])),
                Column::single(col_map().multiplicity),
            )
        })
        .collect()
}

#[must_use]
pub fn filter<F: Field>() -> Column<F> { Column::single(col_map().multiplicity) }

use plonky2::field::types::Field;

use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};
use crate::cross_table_lookup::Column;
use crate::multiplicity_view::MultiplicityView;
use crate::stark::mozak_stark::{RangeCheckTable, Table};

#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct RangeCheckColumnsView<T> {
    /// The limbs (u8) of the u32 value to be range
    /// checked.
    pub limbs: [T; 4],

    /// The u32 value to be range checked and its multiplicity.
    pub multiplicity_view: MultiplicityView<T>,
}
columns_view_impl!(RangeCheckColumnsView);
make_col_map!(RangeCheckColumnsView);

/// Total number of columns for the range check table.
pub(crate) const NUM_RC_COLS: usize = RangeCheckColumnsView::<()>::NUMBER_OF_COLUMNS;

/// Columns containing the data to be range checked in the Mozak
/// [`RangeCheckTable`](crate::cross_table_lookup::RangeCheckTable).
#[must_use]
pub fn data<F: Field>() -> Vec<Column<F>> {
    vec![(0..4)
        .map(|limb| Column::single(col_map().limbs[limb]) * F::from_canonical_u32(1 << (8 * limb)))
        .sum()]
}

#[must_use]
pub fn rangecheck_looking<F: Field>() -> Vec<Table<F>> {
    (0..4)
        .map(|limb| {
            RangeCheckTable::new(
                Column::singles([col_map().limbs[limb]]),
                Column::single(col_map().multiplicity_view.multiplicity),
            )
        })
        .collect()
}

#[must_use]
pub fn filter<F: Field>() -> Column<F> { Column::single(col_map().multiplicity_view.multiplicity) }

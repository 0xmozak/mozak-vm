use crate::columns_view::{columns_view_impl, make_col_map};
use crate::cross_table_lookup::Column;
use crate::stark::mozak_stark::{RangeCheckTable, TableWithTypedOutput};

#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct RangeCheckColumnsView<T> {
    /// The limbs (u8) of the u32 value to be range
    /// checked.
    pub limbs: [T; 4],
    pub multiplicity: T,
}
columns_view_impl!(RangeCheckColumnsView);
make_col_map!(RangeCheckColumnsView);

/// Lookup for columns be range checked in the Mozak
/// [`RangeCheckTable`](crate::cross_table_lookup::RangeCheckTable).
#[must_use]
pub fn lookup() -> TableWithTypedOutput<RangeCheckCtl<Column>> {
    let data = RangeCheckCtl(
        (0..4)
            .map(|limb| COL_MAP.limbs[limb] * (1 << (8 * limb)))
            .sum(),
    );
    RangeCheckTable::new(data, COL_MAP.multiplicity)
}

columns_view_impl!(RangeCheckCtl);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct RangeCheckCtl<T>(pub T);

#[must_use]
pub fn rangecheck_looking() -> Vec<TableWithTypedOutput<RangeCheckCtl<Column>>> {
    (0..4)
        .map(|limb| RangeCheckTable::new(RangeCheckCtl(COL_MAP.limbs[limb]), COL_MAP.multiplicity))
        .collect()
}

use plonky2::field::types::Field;

use crate::columns_view::{columns_view_impl, make_col_map};
use crate::cross_table_lookup::Column;

#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct XorColumnsView<T> {
    /// This column indicates if the row has a corresponding execution row
    /// in the CPU table or if it is a dummy row (which is used to fill the
    /// table to a power of 2).
    pub is_execution_row: T,
    /// This column contains the values in the corresponding row from the CPU
    /// table.
    pub execution: XorView<T>,
    /// This column contains the decomposed limbs of the execution value.
    pub limbs: XorView<[T; 32]>,
}
columns_view_impl!(XorColumnsView);
make_col_map!(XorColumnsView);

#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct XorView<T> {
    pub a: T,
    pub b: T,
    pub out: T,
}
columns_view_impl!(XorView);

/// Columns containing the data which are looked from cpu table into Bitwise
/// stark. [`CpuTable`](crate::cross_table_lookup::CpuTable)
/// [`BitwiseTable`](crate::cross_table_lookup::BitwiseTable).
#[must_use]
pub fn data_for_cpu<F: Field>() -> Vec<Column<F>> { Column::singles(MAP.execution) }

/// Column for a binary filter to indicate a lookup from the
/// [`CpuTable`](crate::cross_table_lookup::CpuTable) in the Mozak
/// [`BitwiseTable`](crate::cross_table_lookup::BitwiseTable).
#[must_use]
pub fn filter_for_cpu<F: Field>() -> Column<F> { Column::single(MAP.is_execution_row) }

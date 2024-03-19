use crate::columns_view::{columns_view_impl, make_col_map};
use crate::cross_table_lookup::Column;
use crate::stark::mozak_stark::{Table, XorTable};

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

/// Lookup between CPU table and Xor stark table.
#[must_use]
pub fn lookup_for_cpu() -> Table {
    XorTable::new(
        Column::singles(col_map().execution),
        Column::single(col_map().is_execution_row),
    )
}

use plonky2::field::types::Field;

use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};
use crate::cross_table_lookup::Column;

columns_view_impl!(MemoryZeroInit);
make_col_map!(MemoryZeroInit);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct MemoryZeroInit<T> {
    pub addr: T,
    pub filter: T,
}

pub const NUM_MEMORYINIT_COLS: usize = MemoryZeroInit::<()>::NUMBER_OF_COLUMNS;

/// Columns containing the data which are looked up from the Memory Table
#[must_use]
pub fn data_for_memory<F: Field>() -> Vec<Column<F>> {
    vec![
        Column::constant(F::ONE), // is_writable
        Column::single(col_map().addr),
        Column::constant(F::ONE),  // clk
        Column::constant(F::ZERO), // value
    ]
}

/// Column for a binary filter to indicate a lookup from the Memory Table
#[must_use]
pub fn filter_for_memory<F: Field>() -> Column<F> { Column::single(col_map().filter) }

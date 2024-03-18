use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};
use crate::cross_table_lookup::Column;
use crate::stark::mozak_stark::{MemoryZeroInitTable, Table};

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
pub fn lookup_for_memory() -> Table {
    MemoryZeroInitTable::new(
        vec![
            Column::constant(1), // is_writable
            Column::single(col_map().addr),
            Column::constant(0), // clk
            Column::constant(0), // value
        ],
        Column::single(col_map().filter),
    )
}

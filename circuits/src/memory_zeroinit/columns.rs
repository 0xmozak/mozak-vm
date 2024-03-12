

use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};
use crate::cross_table_lookup::Column;
use crate::memoryinit::columns::MemoryInitCtl;

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
pub fn data_for_memory() -> MemoryInitCtl<Column> {
    let mem = col_map().map(Column::from);
    MemoryInitCtl {
        is_writable: Column::constant(1),
        address: mem.addr,
        clk: Column::constant(0),
        value: Column::constant(0),
    }
}

/// Column for a binary filter to indicate a lookup from the Memory Table
#[must_use]
pub fn filter_for_memory() -> Column { Column::single(col_map().filter) }

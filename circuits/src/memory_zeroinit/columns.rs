use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};
use crate::cross_table_lookup::ColumnX;
use crate::memoryinit::columns::MemoryInitCtl;

columns_view_impl!(MemoryZeroInit);
make_col_map!(MemoryZeroInit);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct MemoryZeroInit<T> {
    pub addr: T,
    pub filter: T,
}

type ZeroColumn = ColumnX<MemoryZeroInit<i64>>;

pub const NUM_MEMORYINIT_COLS: usize = MemoryZeroInit::<()>::NUMBER_OF_COLUMNS;

/// Columns containing the data which are looked up from the Memory Table
#[must_use]
pub fn data_for_memory() -> MemoryInitCtl<ZeroColumn> {
    let mem = COL_MAP;
    MemoryInitCtl {
        is_writable: ColumnX::constant(1),
        address: mem.addr,
        clk: ColumnX::constant(0),
        value: ColumnX::constant(0),
    }
}

/// Column for a binary filter to indicate a lookup from the Memory Table
#[must_use]
pub fn filter_for_memory() -> ZeroColumn { COL_MAP.filter }

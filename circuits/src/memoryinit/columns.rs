use crate::columns_view::{columns_view_impl, make_col_map};
use crate::cross_table_lookup::Column;

columns_view_impl!(MemElement);
/// A Memory Slot that has an address and a value
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct MemElement<T> {
    pub address: T,
    pub value: T,
}

columns_view_impl!(MemoryInit);
make_col_map!(MemoryInit);
/// A Row of Memory generated from both read-only and read-write memory
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct MemoryInit<T> {
    pub element: MemElement<T>,
    /// Filters out instructions that are duplicates, i.e., appear more than
    /// once in the trace.
    pub filter: T,
    /// 1 if this row is a read-write, 0 if this row is read-only
    pub is_writable: T,
}

/// Columns containing the data which are looked up from the Memory Table
#[must_use]
pub fn data_for_memory() -> Vec<Column> {
    vec![
        Column::single(col_map().is_writable),
        Column::single(col_map().element.address),
        // clk:
        Column::constant(1),
        Column::single(col_map().element.value),
    ]
}

/// Column for a binary filter to indicate a lookup from the Memory Table
#[must_use]
pub fn filter_for_memory() -> Column { Column::single(col_map().filter) }

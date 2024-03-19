use crate::columns_view::{columns_view_impl, make_col_map};
use crate::cross_table_lookup::Column;
use crate::stark::mozak_stark::{Table, TableKind};

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

/// Lookup from the Memory Table
#[must_use]
pub fn lookup_for_memory(kind: TableKind) -> Table {
    Table {
        kind,
        columns: vec![
            Column::single(col_map().is_writable),
            Column::single(col_map().element.address),
            // clk:
            Column::constant(1),
            Column::single(col_map().element.value),
        ],
        filter_column: Column::single(col_map().filter),
    }
}

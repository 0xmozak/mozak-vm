use crate::columns_view::{columns_view_impl, make_col_map};
use crate::cross_table_lookup::ColumnWithTypedInput;
use crate::linear_combination::Column;
use crate::stark::mozak_stark::TableWithUntypedInput;

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

columns_view_impl!(MemoryInitCtl);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct MemoryInitCtl<T> {
    pub is_writable: T,
    pub address: T,
    pub clk: T,
    pub value: T,
}

/// Columns containing the data which are looked up from the Memory Table
#[must_use]
pub fn lookup_for_memory<T>(new: T) -> TableWithUntypedInput<MemoryInitCtl<Column>>
where
    T: Fn(
        MemoryInitCtl<ColumnWithTypedInput<MemoryInit<i64>>>,
        ColumnWithTypedInput<MemoryInit<i64>>,
    ) -> TableWithUntypedInput<MemoryInitCtl<Column>>, {
    let mem = COL_MAP;
    new(
        MemoryInitCtl {
            is_writable: mem.is_writable,
            address: mem.element.address,
            clk: ColumnWithTypedInput::constant(1),
            value: mem.element.value,
        },
        COL_MAP.filter,
    )
}

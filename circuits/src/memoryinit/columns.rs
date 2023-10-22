use plonky2::field::types::Field;

use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};
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
/// A Row of Memomry generated from both read-only and read-write memory
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct MemoryInit<T> {
    pub element: MemElement<T>,
    /// Filters out instructions that are duplicates, i.e., appear more than
    /// once in the trace.
    pub filter: T,
    /// One if this row is a read-write Zero if this row is read-only
    pub is_writable: T,
}

pub const NUM_MEMORYINIT_COLS: usize = MemoryInit::<()>::NUMBER_OF_COLUMNS;

/// Columns containing the data which are looked up from the Memory Table
#[must_use]
pub fn data_for_memory<F: Field>() -> Vec<Column<F>> {
    vec![
        Column::single(MAP.is_writable),
        Column::single(MAP.element.address),
        // clk:
        Column::constant(F::ZERO),
        Column::single(MAP.element.value),
        // is_init:
        Column::constant(F::ONE),
    ]
}

/// Column for a binary filter to indicate a lookup from the Memory Table
#[must_use]
pub fn filter_for_memory<F: Field>() -> Column<F> { Column::single(MAP.filter) }

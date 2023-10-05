use derive_columns_view::ColumnsView;
use plonky2::field::types::Field;

use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};
use crate::cross_table_lookup::Column;

columns_view_impl!(MemElement);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct MemElement<T> {
    pub address: T,
    pub value: T,
}

columns_view_impl!(MemoryInit);
make_col_map!(MemoryInit);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default, ColumnsView)]
pub struct MemoryInit<T> {
    pub element: MemElement<T>,
    pub filter: T,
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

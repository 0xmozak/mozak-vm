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
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct MemoryInit<T> {
    pub element: MemElement<T>,
    pub filter: T,
    pub is_writable: T,
}

pub const NUM_MEMORYINIT_COLS: usize = MemoryInit::<()>::NUMBER_OF_COLUMNS;

#[must_use]
pub fn data_for_ctl<F: Field>() -> Vec<Column<F>> { Column::singles(MAP.element) }

use plonky2::field::types::Field;

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

#[must_use]
pub fn data_for_memory_<F: Field>() -> MemoryInitCtl<Column<F>> {
    let mem = col_map().map(Column::from);
    MemoryInitCtl {
        is_writable: Column::constant(F::ONE),
        address: mem.addr,
        clk: Column::constant(F::ZERO),
        value: Column::constant(F::ZERO),
    }
}

/// Columns containing the data which are looked up from the Memory Table
#[must_use]
pub fn data_for_memory<F: Field>() -> Vec<Column<F>> { data_for_memory_().into_iter().collect() }

/// Column for a binary filter to indicate a lookup from the Memory Table
#[must_use]
pub fn filter_for_memory<F: Field>() -> Column<F> { Column::single(col_map().filter) }

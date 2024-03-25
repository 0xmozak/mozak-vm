use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};
use crate::cross_table_lookup::ColumnWithTypedInput;
use crate::linear_combination::Column;
use crate::memoryinit::columns::MemoryInitCtl;
use crate::stark::mozak_stark::{MemoryZeroInitTable, TableWithUntypedInput};

columns_view_impl!(MemoryZeroInit);
make_col_map!(MemoryZeroInit);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct MemoryZeroInit<T> {
    pub addr: T,
    pub filter: T,
}

pub const NUM_MEMORYINIT_COLS: usize = MemoryZeroInit::<()>::NUMBER_OF_COLUMNS;

/// Lookup into Memory Table
#[must_use]
pub fn lookup_for_memory() -> TableWithUntypedInput<MemoryInitCtl<Column>> {
    let mem = COL_MAP;
    MemoryZeroInitTable::new(
        MemoryInitCtl {
            is_writable: ColumnWithTypedInput::constant(1),
            address: mem.addr,
            clk: ColumnWithTypedInput::constant(0),
            value: ColumnWithTypedInput::constant(0),
        },
        COL_MAP.filter,
    )
}

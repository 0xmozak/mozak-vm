use plonky2::field::types::Field;

use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};
use crate::linear_combination::Column;
use crate::stark::mozak_stark::{MemoryTable, Table};

#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct Memory<T> {
    // Indicates if a row comes from VM execution, or whether it's padding.
    pub is_executed: T,

    // Memory address.
    pub addr: T,

    // Clock at memory access.
    pub clk: T,

    // Opcode of memory access.
    pub op: T,

    // Value of memory access.
    pub value: T,

    // Difference between current and previous address.
    pub diff_addr: T,

    // Inverse of the above column. 0 if the above column is 0.
    pub diff_addr_inv: T,

    // Difference between current and previous clock.
    pub diff_clk: T,
}
columns_view_impl!(Memory);
make_col_map!(Memory);

// Total number of columns.
pub const NUM_MEM_COLS: usize = Memory::<()>::NUMBER_OF_COLUMNS;

#[must_use]
pub fn rangecheck_looking<F: Field>() -> Vec<Table<F>> {
    vec![
        MemoryTable::new(
            Column::singles([MAP.diff_addr]),
            Column::single(MAP.is_executed),
        ),
        MemoryTable::new(
            Column::singles([MAP.diff_clk]),
            Column::single(MAP.is_executed),
        ),
    ]
}

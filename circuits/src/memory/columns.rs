use plonky2::field::types::Field;

use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};
use crate::cross_table_lookup::Column;

#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct Memory<T> {
    /// Indicates if a row comes from VM execution, or whether it's padding.
    pub is_executed: T,

    /// Memory address.
    pub addr: T,

    // Clock at memory access.
    pub clk: T,

    /// Opcode of memory access.
    pub op: T,

    /// Value of memory access.
    pub value: T,

    /// Difference between current and previous address.
    pub diff_addr: T,

    /// Inverse of the above column. 0 if the `diff_addr` is 0.
    pub diff_addr_inv: T,

    /// Difference between current and previous clock.
    pub diff_clk: T,
}
columns_view_impl!(Memory);
make_col_map!(Memory);

/// Total number of columns.
pub const NUM_MEM_COLS: usize = Memory::<()>::NUMBER_OF_COLUMNS;

/// Columns containing the data which are looked from the CPU table into Memory
/// stark table.
#[must_use]
pub fn data_for_cpu<F: Field>() -> Vec<Column<F>> { vec![Column::single(MAP.value)] }

/// Column for a binary filter to indicate a lookup from the CPU table into
/// Memory stark table.
#[must_use]
pub fn filter_for_cpu<F: Field>() -> Column<F> { Column::single(MAP.is_executed) }

use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};

#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct Memory<T> {
    /// Indicates if memory is padding.
    pub padding: T,

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

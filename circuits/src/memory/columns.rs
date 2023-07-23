use crate::utils::{columns_view_impl, make_col_map, NumberOfColumns};

#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub(crate) struct MemoryColumnsView<T: Copy> {
    // Indicates if memory is padding.
    pub(crate) mem_padding: T,

    // Memory address.
    pub(crate) mem_addr: T,

    // Clock at memory access.
    pub(crate) mem_clk: T,

    // Opcode of memory access.
    pub(crate) mem_op: T,

    // Value of memory access.
    pub(crate) mem_value: T,

    // Difference between current and previous address.
    pub(crate) mem_diff_addr: T,

    // Inverse of the above column. 0 if the above column is 0.
    pub(crate) mem_diff_addr_inv: T,

    // Difference between current and previous clock.
    pub(crate) mem_diff_clk: T,
}
columns_view_impl!(MemoryColumnsView);
make_col_map!(MemoryColumnsView);

// Total number of columns.
pub(crate) const NUM_MEM_COLS: usize = MemoryColumnsView::<()>::NUMBER_OF_COLUMNS;

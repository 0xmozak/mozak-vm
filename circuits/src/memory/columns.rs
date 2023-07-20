// Indicates if memory is padding.
pub(crate) const MEM_PADDING: usize = 0;

// Memory address.
pub(crate) const MEM_ADDR: usize = 1;

// Clock at memory access.
pub(crate) const MEM_CLK: usize = 2;

// Opcode of memory access.
pub(crate) const MEM_OP: usize = 3;

// Value of memory access.
pub(crate) const MEM_VALUE: usize = 4;

// Difference between current and previous address.
pub(crate) const MEM_DIFF_ADDR: usize = 5;

// Inverse of the above column. 0 if the above column is 0.
pub(crate) const MEM_DIFF_ADDR_INV: usize = 6;

// Difference between current and previous clock.
pub(crate) const MEM_DIFF_CLK: usize = 7;

// Total number of columns.
pub(crate) const NUM_MEM_COLS: usize = 8;

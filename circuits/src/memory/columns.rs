// Indicates if memory is padding.
pub(crate) const COL_MEM_PADDING: usize = 0;

// Memory address.
pub(crate) const COL_MEM_ADDR: usize = 1;

// Clock at memory access.
pub(crate) const COL_MEM_CLK: usize = 2;

// Opcode of memory access.
pub(crate) const COL_MEM_OP: usize = 3;

// Value of memory access.
pub(crate) const COL_MEM_VALUE: usize = 4;

// Difference between current and previous address.
pub(crate) const COL_MEM_DIFF_ADDR: usize = 5;

// Difference between current and previous clock.
pub(crate) const COL_MEM_DIFF_CLK: usize = 6;

// Total number of columns.
pub(crate) const NUM_MEM_COLS: usize = 7;

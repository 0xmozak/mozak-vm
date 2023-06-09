pub(crate) const COL_MEM_PADDING: usize = 0;
pub(crate) const COL_MEM_ADDR: usize = COL_MEM_PADDING + 1;
pub(crate) const COL_MEM_CLK: usize = COL_MEM_ADDR + 1;
pub(crate) const COL_MEM_OP: usize = COL_MEM_CLK + 1;
pub(crate) const COL_MEM_VALUE: usize = COL_MEM_OP + 1;
pub(crate) const COL_MEM_DIFF_ADDR: usize = COL_MEM_VALUE + 1;
pub(crate) const COL_MEM_DIFF_CLK: usize = COL_MEM_DIFF_ADDR + 1;

pub(crate) const NUM_MEM_COLS: usize = COL_MEM_DIFF_CLK + 1;

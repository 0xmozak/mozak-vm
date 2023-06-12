use std::ops::Range;

pub(crate) const COL_CLK: usize = 0;
pub(crate) const COL_PC: usize = COL_CLK + 1;
pub(crate) const COL_RS1: usize = COL_PC + 1;
pub(crate) const COL_RS2: usize = COL_RS1 + 1;
pub(crate) const COL_RD: usize = COL_RS2 + 1;
pub(crate) const COL_OP1_VALUE: usize = COL_RD + 1;
pub(crate) const COL_OP2_VALUE: usize = COL_OP1_VALUE + 1;
pub(crate) const COL_IMM: usize = COL_OP2_VALUE + 1;
pub(crate) const COL_DST_VALUE: usize = COL_IMM + 1;
pub(crate) const COL_START_REG: usize = COL_DST_VALUE + 1;
pub(crate) const COL_REGS: Range<usize> = COL_START_REG..COL_START_REG + 32;

pub(crate) const COL_S_ADD: usize = COL_REGS.end;
pub(crate) const COL_S_BEQ: usize = COL_S_ADD + 1;
// Column for ECall instruction, but also
// indicates if VM is already halted or not.
pub(crate) const COL_S_HALT: usize = COL_S_BEQ + 1;

pub(crate) const NUM_CPU_COLS: usize = COL_S_HALT + 1;

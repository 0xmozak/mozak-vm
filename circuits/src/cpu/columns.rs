use std::ops::Range;

pub(crate) const COL_CLK: usize = 0;
pub(crate) const COL_PC: usize = COL_CLK + 1;
pub(crate) const COL_RS1: usize = COL_PC + 1;
pub(crate) const COL_RS2: usize = COL_RS1 + 1;
pub(crate) const COL_RD: usize = COL_RS2 + 1;
pub(crate) const COL_START_REG: usize = COL_RD + 1;
pub(crate) const COL_REGS: Range<usize> = COL_START_REG..COL_START_REG + 32;

pub(crate) const COL_SADD: usize = COL_REGS.end;

pub(crate) const NUM_CPU_COLS: usize = COL_SADD + 1;

use std::ops::Range;

use lazy_static::lazy_static;

pub(crate) const COL_CLK: usize = 0;
pub(crate) const COL_PC: usize = COL_CLK + 1;

pub(crate) const COL_RS1_SELECT_START: usize = COL_PC + 1;
pub(crate) const COL_RS1_SELECT_RANGE: Range<usize> =
    COL_RS1_SELECT_START..COL_RS1_SELECT_START + 32;
lazy_static! {
    pub(crate) static ref COL_RS1_SELECT: Vec<usize> = COL_RS1_SELECT_RANGE.collect();
}

pub(crate) const COL_RS2_SELECT_START: usize = COL_RS1_SELECT_RANGE.end + 1;
pub(crate) const COL_RS2_SELECT_RANGE: Range<usize> =
    COL_RS2_SELECT_START..COL_RS2_SELECT_START + 32;
lazy_static! {
    pub(crate) static ref COL_RS2_SELECT: Vec<usize> = COL_RS2_SELECT_RANGE.collect();
}
pub(crate) const COL_RD_SELECT_START: usize = COL_RS2_SELECT_RANGE.end + 1;
pub(crate) const COL_RD_SELECT_RANGE: Range<usize> = COL_RD_SELECT_START..COL_RD_SELECT_START + 32;
lazy_static! {
    pub(crate) static ref COL_RD_SELECT: Vec<usize> = COL_RD_SELECT_RANGE.collect();
}

pub(crate) const COL_OP1_VALUE: usize = COL_RD_SELECT_RANGE.end + 1;
pub(crate) const COL_OP2_VALUE: usize = COL_OP1_VALUE + 1;
pub(crate) const COL_IMM_VALUE: usize = COL_OP2_VALUE + 1;
pub(crate) const COL_DST_VALUE: usize = COL_IMM_VALUE + 1;
pub(crate) const COL_START_REG: usize = COL_DST_VALUE + 1;
pub(crate) const COL_REGS_RANGE: Range<usize> = COL_START_REG..COL_START_REG + 32;
lazy_static! {
    pub(crate) static ref COL_REGS: Vec<usize> = COL_REGS_RANGE.collect();
}

pub(crate) const COL_S_ADD: usize = COL_REGS_RANGE.end;
pub(crate) const COL_S_SUB: usize = COL_S_ADD + 1;
pub(crate) const COL_S_SLT: usize = COL_S_SUB + 1;
pub(crate) const COL_S_SLTU: usize = COL_S_SLT + 1;
pub(crate) const COL_S_BEQ: usize = COL_S_SLTU + 1;
pub(crate) const COL_S_ECALL: usize = COL_S_BEQ + 1;
pub(crate) const COL_S_HALT: usize = COL_S_ECALL + 1;

pub(crate) const COL_S_SLT_SIGN1: usize = COL_S_HALT + 1;
pub(crate) const COL_S_SLT_SIGN2: usize = COL_S_SLT_SIGN1 + 1;
pub(crate) const COL_S_SLT_OP1_VAL_FIXED: usize = COL_S_SLT_SIGN2 + 1;
pub(crate) const COL_S_SLT_OP2_VAL_FIXED: usize = COL_S_SLT_OP1_VAL_FIXED + 1;
pub(crate) const COL_CMP_ABS_DIFF: usize = COL_S_SLT_OP2_VAL_FIXED + 1;
pub(crate) const COL_CMP_DIFF_INV: usize = COL_CMP_ABS_DIFF + 1;
pub(crate) const COL_LESS_THAN: usize = COL_CMP_DIFF_INV + 1;

pub(crate) const NUM_CPU_COLS: usize = COL_LESS_THAN + 1;

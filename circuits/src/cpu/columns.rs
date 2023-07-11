use std::ops::Range;

use itertools::Itertools;
use lazy_static::lazy_static;
use plonky2::field::types::Field;

use crate::cross_table_lookup::Column;

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
pub(crate) const COL_S_DIVU: usize = COL_S_SUB + 1;
pub(crate) const COL_S_REMU: usize = COL_S_DIVU + 1;
pub(crate) const COL_S_MUL: usize = COL_S_REMU + 1;
pub(crate) const COL_S_SLT: usize = COL_S_MUL + 1;
pub(crate) const COL_S_SLTU: usize = COL_S_SLT + 1;
pub(crate) const COL_S_BEQ: usize = COL_S_SLTU + 1;
pub(crate) const COL_S_ECALL: usize = COL_S_BEQ + 1;
pub(crate) const COL_S_HALT: usize = COL_S_ECALL + 1;
pub(crate) const COL_S_RC: usize = COL_S_HALT + 1;

pub(crate) const COL_S_SLT_SIGN1: usize = COL_S_RC + 1;
pub(crate) const COL_S_SLT_SIGN2: usize = COL_S_SLT_SIGN1 + 1;
pub(crate) const COL_S_SLT_OP1_VAL_FIXED: usize = COL_S_SLT_SIGN2 + 1;
pub(crate) const COL_S_SLT_OP2_VAL_FIXED: usize = COL_S_SLT_OP1_VAL_FIXED + 1;
pub(crate) const COL_CMP_ABS_DIFF: usize = COL_S_SLT_OP2_VAL_FIXED + 1;
pub(crate) const COL_CMP_DIFF_INV: usize = COL_CMP_ABS_DIFF + 1;
pub(crate) const COL_LESS_THAN: usize = COL_CMP_DIFF_INV + 1;

pub(crate) const DIVU_QUOTIENT: usize = COL_LESS_THAN + 1;
pub(crate) const DIVU_REMAINDER: usize = DIVU_QUOTIENT + 1;
pub(crate) const DIVU_REMAINDER_SLACK: usize = DIVU_REMAINDER + 1;
pub(crate) const DIVU_Q_INV: usize = DIVU_REMAINDER_SLACK + 1;
pub(crate) const MUL_HIGH_BITS: usize = DIVU_Q_INV + 1;

pub(crate) const NUM_CPU_COLS: usize = MUL_HIGH_BITS + 1;

/// Columns containing the data to be range checked in the Mozak
/// [`CpuTable`](crate::cross_table_lookup::CpuTable).
pub(crate) fn data_for_rangecheck<F: Field>() -> Vec<Column<F>> {
    Column::singles([COL_DST_VALUE]).collect_vec()
}

/// Column for a binary filter for our range check in the Mozak
/// [`CpuTable`](crate::cross_table_lookup::CpuTable).
pub(crate) fn filter_for_rangecheck<F: Field>() -> Column<F> { Column::single(COL_S_RC) }

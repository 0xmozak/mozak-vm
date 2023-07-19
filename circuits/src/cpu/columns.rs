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
pub(crate) const COL_S_XOR: usize = COL_S_SUB + 1;
pub(crate) const COL_S_OR: usize = COL_S_XOR + 1;
pub(crate) const COL_S_AND: usize = COL_S_OR + 1;

pub(crate) const COL_S_DIVU: usize = COL_S_AND + 1;
pub(crate) const COL_S_REMU: usize = COL_S_DIVU + 1;
pub(crate) const COL_S_MUL: usize = COL_S_REMU + 1;
pub(crate) const COL_S_MULHU: usize = COL_S_MUL + 1;
pub(crate) const COL_S_SLL: usize = COL_S_MULHU + 1;
pub(crate) const COL_S_SLT: usize = COL_S_SLL + 1;
pub(crate) const COL_S_SLTU: usize = COL_S_SLT + 1;
pub(crate) const COL_S_SRL: usize = COL_S_SLTU + 1;
pub(crate) const COL_S_JALR: usize = COL_S_SRL + 1;
pub(crate) const COL_S_BEQ: usize = COL_S_JALR + 1;
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
pub(crate) const BRANCH_EQUAL: usize = COL_LESS_THAN + 1;

pub(crate) const XOR_A: usize = BRANCH_EQUAL + 1;
pub(crate) const XOR_B: usize = XOR_A + 1;
pub(crate) const XOR_OUT: usize = XOR_B + 1;

// TODO: for shift operations, we need to hook up POWERS_OF_2_IN and
// POWERS_OF_2_OUT to a cross-table lookup for input values 0..32.
pub(crate) const POWERS_OF_2_IN: usize = XOR_OUT + 1;
pub(crate) const POWERS_OF_2_OUT: usize = POWERS_OF_2_IN + 1;

pub(crate) const QUOTIENT: usize = POWERS_OF_2_OUT + 1;
pub(crate) const REMAINDER: usize = QUOTIENT + 1;
pub(crate) const REMAINDER_SLACK: usize = REMAINDER + 1;
pub(crate) const DIVISOR_INV: usize = REMAINDER_SLACK + 1;

pub(crate) const DIVISOR: usize = DIVISOR_INV + 1;

// TODO: PRODUCT_LOW_BITS and PRODUCT_HIGH_BITS need range checking.
pub(crate) const MULTIPLIER: usize = DIVISOR + 1;
pub(crate) const PRODUCT_LOW_BITS: usize = MULTIPLIER + 1;
pub(crate) const PRODUCT_HIGH_BITS: usize = PRODUCT_LOW_BITS + 1;
pub(crate) const PRODUCT_HIGH_DIFF_INV: usize = PRODUCT_HIGH_BITS + 1;

// TODO: In future we may want to merge BRANCH_DIFF_INV and COL_CMP_DIFF_INV
pub(crate) const BRANCH_DIFF_INV: usize = PRODUCT_HIGH_BITS + 1;
pub(crate) const NUM_CPU_COLS: usize = BRANCH_DIFF_INV + 1;

/// Columns containing the data to be range checked in the Mozak
/// [`CpuTable`](crate::cross_table_lookup::CpuTable).
pub(crate) fn data_for_rangecheck<F: Field>() -> Vec<Column<F>> {
    Column::singles([COL_DST_VALUE]).collect_vec()
}

/// Column for a binary filter for our range check in the Mozak
/// [`CpuTable`](crate::cross_table_lookup::CpuTable).
pub(crate) fn filter_for_rangecheck<F: Field>() -> Column<F> { Column::single(COL_S_RC) }

use std::ops::Range;

use itertools::Itertools;
use lazy_static::lazy_static;
use plonky2::field::types::Field;

use crate::cross_table_lookup::Column;

pub(crate) const CLK: usize = 0;
pub(crate) const PC: usize = CLK + 1;

pub(crate) const RS1_SELECT_START: usize = PC + 1;
pub(crate) const RS1_SELECT_RANGE: Range<usize> = RS1_SELECT_START..RS1_SELECT_START + 32;
lazy_static! {
    pub(crate) static ref RS1_SELECT: Vec<usize> = RS1_SELECT_RANGE.collect();
}

pub(crate) const RS2_SELECT_START: usize = RS1_SELECT_RANGE.end + 1;
pub(crate) const RS2_SELECT_RANGE: Range<usize> = RS2_SELECT_START..RS2_SELECT_START + 32;
lazy_static! {
    pub(crate) static ref RS2_SELECT: Vec<usize> = RS2_SELECT_RANGE.collect();
}
pub(crate) const RD_SELECT_START: usize = RS2_SELECT_RANGE.end + 1;
pub(crate) const RD_SELECT_RANGE: Range<usize> = RD_SELECT_START..RD_SELECT_START + 32;
lazy_static! {
    pub(crate) static ref RD_SELECT: Vec<usize> = RD_SELECT_RANGE.collect();
}

pub(crate) const OP1_VALUE: usize = RD_SELECT_RANGE.end + 1;
// OP2_VALUE is the sum of the value of the second operand register and the
// immediate value.
pub(crate) const OP2_VALUE: usize = OP1_VALUE + 1;
pub(crate) const IMM_VALUE: usize = OP2_VALUE + 1;
pub(crate) const DST_VALUE: usize = IMM_VALUE + 1;
pub(crate) const BRANCH_TARGET: usize = DST_VALUE + 1;
pub(crate) const START_REG: usize = BRANCH_TARGET + 1;
pub(crate) const REGS_RANGE: Range<usize> = START_REG..START_REG + 32;
lazy_static! {
    pub(crate) static ref REGS: Vec<usize> = REGS_RANGE.collect();
}

pub(crate) const S_ADD: usize = REGS_RANGE.end;
pub(crate) const S_SUB: usize = S_ADD + 1;
pub(crate) const S_XOR: usize = S_SUB + 1;
pub(crate) const S_OR: usize = S_XOR + 1;
pub(crate) const S_AND: usize = S_OR + 1;

pub(crate) const S_DIVU: usize = S_AND + 1;
pub(crate) const S_REMU: usize = S_DIVU + 1;
pub(crate) const S_MUL: usize = S_REMU + 1;
pub(crate) const S_MULHU: usize = S_MUL + 1;
pub(crate) const S_SLL: usize = S_MULHU + 1;
pub(crate) const S_SLT: usize = S_SLL + 1;
pub(crate) const S_SLTU: usize = S_SLT + 1;
pub(crate) const S_SRL: usize = S_SLTU + 1;
pub(crate) const S_JALR: usize = S_SRL + 1;
pub(crate) const S_BEQ: usize = S_JALR + 1;
pub(crate) const S_BNE: usize = S_BEQ + 1;
pub(crate) const S_ECALL: usize = S_BNE + 1;
pub(crate) const S_HALT: usize = S_ECALL + 1;
pub(crate) const S_RC: usize = S_HALT + 1;

pub(crate) const OP1_SIGN: usize = S_RC + 1;
pub(crate) const OP2_SIGN: usize = OP1_SIGN + 1;
pub(crate) const OP1_VAL_FIXED: usize = OP2_SIGN + 1;
pub(crate) const OP2_VAL_FIXED: usize = OP1_VAL_FIXED + 1;
pub(crate) const CMP_ABS_DIFF: usize = OP2_VAL_FIXED + 1;
pub(crate) const CMP_DIFF_INV: usize = CMP_ABS_DIFF + 1;
pub(crate) const LESS_THAN: usize = CMP_DIFF_INV + 1;
pub(crate) const BRANCH_EQUAL: usize = LESS_THAN + 1;

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

pub(crate) const NUM_CPU_COLS: usize = PRODUCT_HIGH_DIFF_INV + 1;

/// Columns containing the data to be range checked in the Mozak
/// [`CpuTable`](crate::cross_table_lookup::CpuTable).
pub(crate) fn data_for_rangecheck<F: Field>() -> Vec<Column<F>> {
    Column::singles([DST_VALUE]).collect_vec()
}

/// Column for a binary filter for our range check in the Mozak
/// [`CpuTable`](crate::cross_table_lookup::CpuTable).
pub(crate) fn filter_for_rangecheck<F: Field>() -> Column<F> { Column::single(S_RC) }

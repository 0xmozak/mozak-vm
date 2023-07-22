use itertools::Itertools;
use std::mem::transmute;
use plonky2::field::types::Field;

use crate::cross_table_lookup::Column;

use crate::utils::{
    boilerplate_implementations, indices_arr, transmute_without_compile_time_size_checks,
    NumberOfColumns,
};

#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub(crate) struct OpSelectorView<T: Copy> {
    pub add: T,
    pub sub: T,
    pub xor: T,
    pub or: T,
    pub and: T,

    pub divu: T,
    pub remu: T,
    pub mul: T,
    pub mulhu: T,
    pub sll: T,
    pub slt: T,
    pub sltu: T,
    pub srl: T,
    pub jalr: T,
    pub beq: T,
    pub bne: T,
    pub ecall: T,
    pub halt: T,
}

boilerplate_implementations!(CpuColumnsView);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub(crate) struct CpuColumnsView<T: Copy> {
    pub clk: T,
    pub pc: T,
    
    pub rs1_select: [T; 32],
    pub rs2_select: [T; 32],
    pub rd_select: [T; 32],
    
    pub op1_value: T,
    pub op2_value: T,
    pub imm_value: T,
    pub dst_value: T,

    pub regs: [T; 32],

    pub ops: OpSelectorView<T>,
    pub rc: T,

    pub op1_sign: T,
    pub op2_sign: T,
    pub op1_val_fixed: T,
    pub op2_val_fixed: T,
    pub cmp_abs_diff: T,
    pub cmp_diff_inv: T,
    pub less_than: T,
    pub branch_equal: T,
    
    pub xor_a: T,
    pub xor_b: T,
    pub xor_out: T,
    
    // TODO: for shift operations, we need to hook up POWERS_OF_2_IN and
    // POWERS_OF_2_OUT to a cross-table lookup for input values 0..32.
    pub powers_of_2_in: T,
    pub powers_of_2_out: T,
    
    pub quotient: T,
    pub remainder: T,
    pub remainder_slack: T,
    pub divisor_inv: T,    
    pub divisor: T,
    
    // TODO: PRODUCT_LOW_BITS and PRODUCT_HIGH_BITS need range checking.
    pub multiplier: T,
    pub product_low_bits: T,
    pub product_high_bits: T,
    pub product_high_diff_inv: T,
    
    // TODO: In future we may want to merge BRANCH_DIFF_INV and CMP_DIFF_INV
    pub branch_diff_inv: T,
}

// TODO: re-use this logic for CPU col map.
pub(crate) const COL_MAP: CpuColumnsView<usize> = {
    const COLUMNS: usize = CpuColumnsView::<()>::NUMBER_OF_COLUMNS;
    let indices_arr = indices_arr::<COLUMNS>();
    unsafe { transmute::<[usize; COLUMNS], CpuColumnsView<usize>>(indices_arr) }
};

pub const NUM_CPU_COLS: usize = CpuColumnsView::<()>::NUMBER_OF_COLUMNS;

/// Columns containing the data to be range checked in the Mozak
/// [`CpuTable`](crate::cross_table_lookup::CpuTable).
pub(crate) fn data_for_rangecheck<F: Field>() -> Vec<Column<F>> {
    Column::singles([COL_MAP.dst_value]).collect_vec()
}

/// Column for a binary filter for our range check in the Mozak
/// [`CpuTable`](crate::cross_table_lookup::CpuTable).
pub(crate) fn filter_for_rangecheck<F: Field>() -> Column<F> { Column::single(COL_MAP.rc) }

use itertools::Itertools;
use plonky2::field::types::Field;

use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};
use crate::cross_table_lookup::Column;

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
    pub blt: T,
    pub bltu: T,
    pub bge: T,
    pub bgeu: T,
    pub ecall: T,
    pub halt: T,
}

columns_view_impl!(CpuColumnsView);
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
    pub branch_target: T,

    pub regs: [T; 32],

    pub ops: OpSelectorView<T>,

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
}

make_col_map!(CpuColumnsView);

pub const NUM_CPU_COLS: usize = CpuColumnsView::<()>::NUMBER_OF_COLUMNS;

/// Columns containing the data to be range checked in the Mozak
/// [`CpuTable`](crate::cross_table_lookup::CpuTable).
pub(crate) fn data_for_rangecheck<F: Field>() -> Vec<Column<F>> {
    Column::singles([MAP.dst_value]).collect_vec()
}

/// Column for a binary filter for our range check in the Mozak
/// [`CpuTable`](crate::cross_table_lookup::CpuTable).
pub(crate) fn filter_for_rangecheck<F: Field>() -> Column<F> { Column::single(MAP.ops.add) }

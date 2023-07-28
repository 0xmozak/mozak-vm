use core::ops::Range;

use itertools::Itertools;
use plonky2::field::types::Field;

use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};
use crate::cross_table_lookup::Column;

#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct ShiftAmountView<T> {
    pub is_executed: T,
    pub shamt: T,
    pub multiplier: T,
    pub fixed_shamt: T,
    pub fixed_multiplier: T,
    pub shamt_permuted: T,
    pub multiplier_permuted: T,
    pub fixed_shamt_permuted: T,
    pub fixed_multiplier_permuted: T,
}

pub const FIXED_SHAMT_RANGE: Range<u8> = 0..32;
columns_view_impl!(ShiftAmountView);
make_col_map!(ShiftAmountView);

// Total number of columns.
pub const NUM_SHAMT_COLS: usize = ShiftAmountView::<()>::NUMBER_OF_COLUMNS;

/// Columns containing data from CPU table.
#[must_use]
pub fn data_for_cpu<F: Field>() -> Vec<Column<F>> {
    Column::singles([MAP.shamt, MAP.multiplier]).collect_vec()
}

/// Column containing filter from CPU table.
#[must_use]
pub fn filter_for_cpu<F: Field>() -> Column<F> { Column::single(MAP.is_executed) }

/// Columns containing the permuted data of executed instructions.
#[must_use]
pub fn data_for_inst<F: Field>() -> Vec<Column<F>> {
    Column::singles([MAP.shamt_permuted, MAP.multiplier_permuted]).collect_vec()
}

/// Columns containing the permuted data for fixed values.
#[must_use]
pub fn data_for_fixed_value<F: Field>() -> Vec<Column<F>> {
    Column::singles([MAP.fixed_shamt_permuted, MAP.fixed_multiplier_permuted]).collect_vec()
}

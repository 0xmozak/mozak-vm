use core::ops::Range;

use itertools::Itertools;
use plonky2::field::types::Field;

use crate::columns_view::{columns_view_impl, make_col_map};
use crate::cross_table_lookup::Column;

columns_view_impl!(Bitshift);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct Bitshift<T> {
    pub amount: T,
    pub multiplier: T,
}

impl From<u64> for Bitshift<u64> {
    fn from(amount: u64) -> Self {
        Self {
            amount,
            multiplier: 1 << amount,
        }
    }
}

make_col_map!(ShiftAmountView);
columns_view_impl!(ShiftAmountView);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct ShiftAmountView<T> {
    pub is_executed: T,
    pub executed: Bitshift<T>,
}

pub const FIXED_SHAMT_RANGE: Range<u64> = 0..32;

/// Columns containing data from CPU table.
#[must_use]
pub fn data_for_cpu<F: Field>() -> Vec<Column<F>> { Column::singles(MAP.executed).collect_vec() }

/// Column containing filter from CPU table.
#[must_use]
pub fn filter_for_cpu<F: Field>() -> Column<F> { Column::single(MAP.is_executed) }

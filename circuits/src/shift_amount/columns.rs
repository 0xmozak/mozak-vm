use core::ops::Range;

use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};

#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct ShiftAmountView<T: Copy> {
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

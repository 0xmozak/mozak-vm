use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};

columns_view_impl!(RegisterInit);
make_col_map!(RegisterInit);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct RegisterInit<T> {
    /// The 'address' that indexes into 1 of our 32 registers. Should only
    /// take values 0-31, so this column should be a running sum
    /// from 0 to 31 (inclusive).
    pub reg_addr: T,

    /// Value of the register.
    pub value: T,

    /// Binary column that marks a row as a dummy to exclude from cross table
    /// lookups. In our design, this should be r0, which should always
    /// be 0, so `is_dummy` should be 1 for the first row.
    pub is_dummy: T,
}

pub const NUM_REGISTERINIT_COLS: usize = RegisterInit::<()>::NUMBER_OF_COLUMNS;

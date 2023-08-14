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

make_col_map!(BitshiftView);
columns_view_impl!(BitshiftView);
/// `is_executed` column tells if the corresponding row in the CPU table
/// has been executed with the `SHL` or `SHR` operation.
/// `executed` contains the `Bitshift` columns with the shift amount
/// and the multiplier.
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct BitshiftView<T> {
    pub is_executed: T,
    pub executed: Bitshift<T>,
}

/// Columns containing data from CPU table.
#[must_use]
pub fn data_for_cpu<F: Field>() -> Vec<Column<F>> { Column::singles(MAP.executed) }

/// Column containing filter from CPU table.
#[must_use]
pub fn filter_for_cpu<F: Field>() -> Column<F> { Column::single(MAP.is_executed) }

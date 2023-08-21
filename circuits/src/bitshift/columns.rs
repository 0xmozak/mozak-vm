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
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct BitshiftView<T> {
    /// This column tells if the row has a corresponding value row
    /// in the CPU table. If not, then this is a padding row, used to
    /// pad the table to a power of 2 size or a dummy row
    /// to bridge a gap in the shift amounts.
    pub is_executed: T,
    /// Contains the `Bitshift` columns with the shift amount and the
    /// multiplier.
    pub executed: Bitshift<T>,
}

/// Columns containing data from CPU table.
#[must_use]
pub fn data_for_cpu<F: Field>() -> Vec<Column<F>> { Column::singles(MAP.executed) }

/// Column containing filter from CPU table.
#[must_use]
pub fn filter_for_cpu<F: Field>() -> Column<F> { Column::single(MAP.is_executed) }

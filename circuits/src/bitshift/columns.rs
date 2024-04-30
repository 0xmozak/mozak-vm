use crate::columns_view::{columns_view_impl, make_col_map};
use crate::linear_combination::Column;
use crate::stark::mozak_stark::{BitshiftTable, TableWithTypedOutput};

columns_view_impl!(Bitshift);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct Bitshift<T> {
    pub amount: T,
    pub multiplier: T,
}

impl From<u8> for Bitshift<u32> {
    fn from(amount: u8) -> Self {
        Self {
            amount: amount.into(),
            multiplier: 1 << amount,
        }
    }
}

make_col_map!(BitshiftView);
columns_view_impl!(BitshiftView);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct BitshiftView<T> {
    /// Contains the `Bitshift` columns with the shift amount and the
    /// multiplier.
    pub executed: Bitshift<T>,
    /// This column tells if the row has a corresponding value row
    /// in the CPU table. If not, then this is a padding row, used to
    /// pad the table to a power of 2 size or a dummy row
    /// to bridge a gap in the shift amounts.
    /// For logup, this can be used to track multiplicity
    pub multiplicity: T,
}

/// Lookup from the CPU table into Bitshift stark table.
#[must_use]
pub fn lookup_for_cpu() -> TableWithTypedOutput<Bitshift<Column>> {
    BitshiftTable::new(COL_MAP.executed, COL_MAP.multiplicity)
}

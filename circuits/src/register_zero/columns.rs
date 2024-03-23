use plonky2::hash::hash_types::RichField;

use crate::columns_view::{columns_view_impl, make_col_map};
#[cfg(feature = "enable_register_starks")]
use crate::linear_combination::Column;
use crate::linear_combination_typed::ColumnWithTypedInput;
use crate::register::columns::RegisterCtl;
use crate::stark::mozak_stark::RegisterZeroTable;
#[cfg(feature = "enable_register_starks")]
use crate::stark::mozak_stark::TableWithTypedOutput;

columns_view_impl!(RegisterZero);
make_col_map!(RegisterZero);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
/// The columns of the register 0 table.
/// Register 0 is a special register that is always 0.
/// Thus we don't need neither a value column nor a register address column.
pub struct RegisterZero<T> {
    /// The register 'address' that indexes into 1 of our 32 registers.
    /// Should only take values 0-31, so this column should be a running sum
    /// from 0 to 31 (inclusive). Note that this isn't the same as memory
    /// address.
    pub clk: T,

    /// Columns that indicate what action is taken on the register.
    pub op: T,

    pub is_used: T,
}

impl<F: RichField + core::fmt::Debug> From<RegisterCtl<F>> for RegisterZero<F> {
    fn from(ctl: RegisterCtl<F>) -> Self {
        RegisterZero {
            clk: ctl.clk,
            op: ctl.op,
            is_used: F::ONE,
        }
    }
}

#[must_use]
pub fn register_looked() -> TableWithTypedOutput<RegisterCtl<Column>> {
    let reg = COL_MAP;
    RegisterZeroTable::new(
        RegisterCtl {
            clk: reg.clk,
            op: reg.op,
            addr: ColumnWithTypedInput::constant(0),
            value: ColumnWithTypedInput::constant(0),
        },
        // TODO: We can probably do the register init in the same lookup?
        reg.is_used,
    )
}

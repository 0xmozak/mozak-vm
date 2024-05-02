use plonky2::hash::hash_types::RichField;

use crate::columns_view::{columns_view_impl, make_col_map};
use crate::linear_combination::Column;
use crate::linear_combination_typed::ColumnWithTypedInput;
use crate::register::general::columns::Register;
use crate::register::RegisterCtl;
use crate::stark::mozak_stark::{RegisterZeroWriteTable, TableWithTypedOutput};

columns_view_impl!(RegisterZeroWrite);
make_col_map!(RegisterZeroWrite);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
/// The columns of the register 0 table.
/// Register 0 is a special register that is always 0.
/// Thus we don't need neither a value column nor a register address column.
pub struct RegisterZeroWrite<T> {
    pub clk: T,

    /// Value of the register at time (in clk) of access.
    /// We accept writes for any value, but reads and inits will always be 0.
    pub value: T,

    pub is_used: T,
}

impl<F: RichField + core::fmt::Debug> From<Register<F>> for RegisterZeroWrite<F> {
    fn from(ctl: Register<F>) -> Self {
        RegisterZeroWrite {
            clk: ctl.clk,
            value: ctl.value,
            is_used: F::ONE,
        }
    }
}

#[must_use]
pub fn register_looked() -> TableWithTypedOutput<RegisterCtl<Column>> {
    RegisterZeroWriteTable::new(
        RegisterCtl {
            clk: COL_MAP.clk,
            op: ColumnWithTypedInput::constant(2), // write
            addr: ColumnWithTypedInput::constant(0),
            value: COL_MAP.value,
        },
        COL_MAP.is_used,
    )
}

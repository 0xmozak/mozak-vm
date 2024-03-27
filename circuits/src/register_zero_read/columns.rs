use plonky2::hash::hash_types::RichField;

use crate::columns_view::{columns_view_impl, make_col_map};
use crate::linear_combination::Column;
use crate::linear_combination_typed::ColumnWithTypedInput;
use crate::register::columns::{Register, RegisterCtl};
use crate::stark::mozak_stark::{RegisterZeroReadTable, TableWithTypedOutput};

columns_view_impl!(RegisterZeroRead);
make_col_map!(RegisterZeroRead);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
/// The columns of the register 0 table.
/// Register 0 is a special register that is always 0.
/// Thus we don't need neither a value column nor a register address column.
pub struct RegisterZeroRead<T> {
    pub clk: T,
    pub is_used: T,
}

impl<F: RichField + core::fmt::Debug> From<Register<F>> for RegisterZeroRead<F> {
    fn from(ctl: Register<F>) -> Self {
        RegisterZeroRead {
            clk: ctl.clk,
            is_used: F::ONE,
        }
    }
}

#[must_use]
pub fn register_looked() -> TableWithTypedOutput<RegisterCtl<Column>> {
    let reg = COL_MAP;
    RegisterZeroReadTable::new(
        RegisterCtl {
            clk: reg.clk,
            op: ColumnWithTypedInput::constant(1),
            addr: ColumnWithTypedInput::constant(0),
            value: ColumnWithTypedInput::constant(0),
        },
        reg.is_used,
    )
}

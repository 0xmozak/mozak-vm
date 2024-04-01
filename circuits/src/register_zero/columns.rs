use plonky2::hash::hash_types::RichField;

use crate::columns_view::{columns_view_impl, make_col_map};
use crate::generation::instruction::ascending_sum;
use crate::linear_combination::Column;
use crate::linear_combination_typed::ColumnWithTypedInput;
use crate::register::columns::{Register, RegisterCtl};
use crate::stark::mozak_stark::{RegisterZeroTable, TableWithTypedOutput};

columns_view_impl!(RegisterZero);
make_col_map!(RegisterZero);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
/// The columns of the register 0 table.
/// Register 0 is a special register that is always 0.
/// Thus we don't need neither a value column nor a register address column.
pub struct RegisterZero<T> {
    pub clk: T,

    /// Value of the register at time (in clk) of access.
    /// We accept writes for any value, but reads and inits will always be 0.
    pub value: T,

    /// Columns that indicate what action is taken on the register.
    pub op: T,

    pub is_used: T,
}

impl<F: RichField + core::fmt::Debug> From<Register<F>> for RegisterZero<F> {
    fn from(ctl: Register<F>) -> Self {
        RegisterZero {
            clk: ctl.clk,
            value: ctl.value,
            op: ascending_sum(ctl.ops),
            is_used: F::ONE,
        }
    }
}

#[must_use]
pub fn register_looked() -> TableWithTypedOutput<RegisterCtl<Column>> {
    RegisterZeroTable::new(
        RegisterCtl {
            clk: COL_MAP.clk,
            op: COL_MAP.op,
            addr: ColumnWithTypedInput::constant(0),
            value: COL_MAP.value,
        },
        COL_MAP.is_used,
    )
}

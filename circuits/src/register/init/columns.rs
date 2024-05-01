use crate::columns_view::{columns_view_impl, make_col_map};
use crate::linear_combination::Column;
use crate::linear_combination_typed::ColumnWithTypedInput;
use crate::register::RegisterCtl;
use crate::stark::mozak_stark::{RegisterInitTable, TableWithTypedOutput};

columns_view_impl!(RegisterInit);
make_col_map!(RegisterInit);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct RegisterInit<T> {
    /// The 'address' that indexes into 1 of our 32 registers. Should only
    /// take values 0-31, so this column should be a running sum
    /// from 0 to 31 (inclusive).
    pub reg_addr: T,

    /// Value of the register.
    pub value: T,
}

#[must_use]
pub fn lookup_for_register() -> TableWithTypedOutput<RegisterCtl<Column>> {
    RegisterInitTable::new(
        RegisterCtl {
            clk: ColumnWithTypedInput::constant(0),
            op: ColumnWithTypedInput::constant(0),
            addr: COL_MAP.reg_addr,
            value: COL_MAP.value,
        },
        ColumnWithTypedInput::constant(1),
    )
}

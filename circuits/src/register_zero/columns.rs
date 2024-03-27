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
    /// Combined column for the clock and the operation.
    pub aug_clk: T,

    /// Value of the register at time (in clk) of access.
    /// We accept writes for any value, but reads and inits will always be 0.
    pub value: T,

    pub is_used: T,
}

// clk: 32 bit-ish;  Probably less, actually, do we care?
// value: 32 bit;
// op: 0,1,2 (or 3?) -> 2 bit.
// 63 bit?
//
// 29 bit for clock, if we do this.
// value + op can go together!
//
// is_used: 1 bit, but we could also pad extra reads into some other table?  Not
// sure.  We probably need it.

impl<F: RichField + core::fmt::Debug> From<Register<F>> for RegisterZero<F> {
    fn from(ctl: Register<F>) -> Self {
        RegisterZero {
            aug_clk: ctl.clk * F::from_canonical_u8(3) + ascending_sum(ctl.ops),
            value: ctl.value,
            is_used: F::ONE,
        }
    }
}

#[must_use]
pub fn register_looked() -> TableWithTypedOutput<RegisterCtl<Column>> {
    let reg = COL_MAP;
    RegisterZeroTable::new(
        RegisterCtl {
            // TODO: we can fix this.
            aug_clk: reg.aug_clk,
            addr: ColumnWithTypedInput::constant(0),
            value: reg.value,
        },
        reg.is_used,
    )
}

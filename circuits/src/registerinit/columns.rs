use crate::columns_view::{columns_view_impl, make_col_map};
#[cfg(feature = "enable_register_starks")]
use crate::linear_combination::Column;
#[cfg(feature = "enable_register_starks")]
use crate::stark::mozak_stark::{RegisterInitTable, TableWithTypedOutput};

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

    /// Binary column that marks a register as used to include in cross table
    /// lookups against `RegisterStark`'s `is_init` column. This also serves as
    /// an implicit range check on our register addresses.
    ///
    /// In our design, r0 should always be unused, so it's always 0.
    /// The other registers (r1-r31) should all be 1.
    pub is_looked_up: T,
}

columns_view_impl!(RegisterInitCtl);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct RegisterInitCtl<T> {
    pub addr: T,
    pub value: T,
}

#[cfg(feature = "enable_register_starks")]
#[must_use]
pub fn lookup_for_register() -> TableWithTypedOutput<RegisterInitCtl<Column>> {
    let reg = COL_MAP;
    RegisterInitTable::new(
        RegisterInitCtl {
            addr: reg.reg_addr,
            value: reg.value,
        },
        reg.is_looked_up,
    )
}

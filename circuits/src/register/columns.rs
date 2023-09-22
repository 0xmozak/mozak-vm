use crate::columns_view::{columns_view_impl, make_col_map};

columns_view_impl!(Register);
make_col_map!(Register);

/// [`Design doc for RegisterSTARK`](https://www.notion.so/0xmozak/Register-File-STARK-62459d68aea648a0abf4e97aa0093ea2?pvs=4#0729f89ddc724967ac991c9e299cc4fc)
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct Register<T> {
    /// The 'address' that indexes into 1 of our 32 registers. Should only
    /// take values 0-31, so this column should be a running sum
    /// from 0 to 31 (inclusive).
    pub reg_addr: T,

    /// Binary filter column that marks a row if the register 'address' is
    /// different from the previous row, marking a change in address.
    // TODO: think about if `is_init` column is sufficient to act as a
    // marker for if address changed
    pub did_addr_change: T,

    /// Value of the register at time (in clk) of access.
    pub value: T,

    /// Augmented clock at register access. This is calculated as:
    /// augmented_clk = clk * 2 for register reads, and
    /// augmented_clk = clk * 2 + 1 for register writes,
    /// to ensure that we do not write to the register before we read.
    pub augmented_clk: T,

    /// Binary filter column that marks a row as the initialization of
    /// a register.
    pub is_init: T,

    /// Binary filter column that marks a row as a register read.
    pub is_read: T,

    /// Binary filter column that marks a row as a register write.
    pub is_write: T,
}

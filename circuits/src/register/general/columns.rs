use core::ops::Add;

use plonky2::hash::hash_types::RichField;

use crate::columns_view::{columns_view_impl, make_col_map};
use crate::generation::instruction::ascending_sum;
use crate::linear_combination::Column;
use crate::linear_combination_typed::ColumnWithTypedInput;
use crate::rangecheck::columns::RangeCheckCtl;
use crate::register::RegisterCtl;
use crate::stark::mozak_stark::{RegisterTable, TableWithTypedOutput};

columns_view_impl!(Ops);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct Ops<T> {
    /// Binary filter column that marks a row as the initialization of
    /// a register.
    pub is_init: T,

    /// Binary filter column that marks a row as a register read.
    pub is_read: T,

    /// Binary filter column that marks a row as a register write.
    pub is_write: T,
}

impl<F: RichField> From<F> for Ops<F> {
    fn from(f: F) -> Self {
        match f.to_noncanonical_u64() {
            0 => Self::init(),
            1 => Self::read(),
            2 => Self::write(),
            _ => panic!("Invalid ops value: {f:?}"),
        }
    }
}

impl<F: RichField> Ops<F> {
    #[must_use]
    pub fn to_field(self) -> F { ascending_sum(self) }

    #[must_use]
    pub fn init() -> Self {
        Self {
            is_init: F::ONE,
            ..Default::default()
        }
    }

    #[must_use]
    pub fn read() -> Self {
        Self {
            is_read: F::ONE,
            ..Default::default()
        }
    }

    #[must_use]
    pub fn write() -> Self {
        Ops {
            is_write: F::ONE,
            ..Default::default()
        }
    }
}

columns_view_impl!(Register);
make_col_map!(Register);
/// [`Design doc for RegisterSTARK`](https://www.notion.so/0xmozak/Register-File-STARK-62459d68aea648a0abf4e97aa0093ea2?pvs=4#0729f89ddc724967ac991c9e299cc4fc)
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct Register<T> {
    /// The register 'address' that indexes into 1 of our 32 registers.
    /// Should only take values 0-31, so this column should be a running sum
    /// from 0 to 31 (inclusive). Note that this isn't the same as memory
    /// address.
    pub addr: T,

    /// Value of the register at time (in clk) of access.
    pub value: T,

    pub clk: T,

    /// Columns that indicate what action is taken on the register.
    pub ops: Ops<T>,
}

impl<F: RichField + core::fmt::Debug> From<RegisterCtl<F>> for Register<F> {
    fn from(ctl: RegisterCtl<F>) -> Self {
        Register {
            clk: ctl.clk,
            addr: ctl.addr,
            value: ctl.value,
            ops: Ops::from(ctl.op),
        }
    }
}

/// We create a virtual column known as `is_used`, which flags a row as
/// being 'used' if any one of the ops columns are turned on.
/// This is to differentiate between real rows and padding rows.
impl<T: Add<Output = T> + Copy> Register<T> {
    pub fn is_used(self) -> T { self.ops.is_init + self.ops.is_read + self.ops.is_write }

    pub fn is_rw(self) -> T { self.ops.is_read + self.ops.is_write }

    // See, if we want to add a Mul constraint, we need to add a Mul trait bound?
    // Or whether we want to keep manual addition and clone?
    pub fn augmented_clk(self) -> T { self.clk + self.clk + self.ops.is_write }
}

#[must_use]
pub fn register_looked() -> TableWithTypedOutput<RegisterCtl<Column>> {
    RegisterTable::new(
        RegisterCtl {
            clk: COL_MAP.clk,
            op: ColumnWithTypedInput::ascending_sum(COL_MAP.ops),
            addr: COL_MAP.addr,
            value: COL_MAP.value,
        },
        COL_MAP.ops.is_read + COL_MAP.ops.is_write + COL_MAP.ops.is_init,
    )
}

#[must_use]
pub fn rangecheck_looking() -> Vec<TableWithTypedOutput<RangeCheckCtl<Column>>> {
    vec![RegisterTable::new(
        RangeCheckCtl(COL_MAP.augmented_clk().diff()),
        COL_MAP.is_rw().flip(),
    )]
}

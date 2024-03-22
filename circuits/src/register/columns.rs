use core::ops::Add;

use plonky2::field::types::Field;
use plonky2::hash::hash_types::RichField;

use crate::columns_view::{columns_view_impl, make_col_map};
#[cfg(feature = "enable_register_starks")]
use crate::linear_combination::Column;
#[cfg(feature = "enable_register_starks")]
use crate::rangecheck::columns::RangeCheckCtl;
#[cfg(feature = "enable_register_starks")]
use crate::registerinit::columns::RegisterInitCtl;
#[cfg(feature = "enable_register_starks")]
use crate::stark::mozak_stark::{RegisterTable, TableWithTypedOutput};

columns_view_impl!(Ops);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
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

impl<T: RichField> Ops<T> {
    #[must_use]
    pub fn init() -> Self {
        Self {
            is_init: T::ONE,
            ..Default::default()
        }
    }

    #[must_use]
    pub fn read() -> Self {
        Self {
            is_read: T::ONE,
            ..Default::default()
        }
    }

    #[must_use]
    pub fn write() -> Self {
        Ops {
            is_write: T::ONE,
            ..Default::default()
        }
    }
}

/// Create a dummy [`Ops`]
///
/// We want these 3 filter columns = 0,
/// so we can constrain `is_used = is_init + is_read + is_write`.
#[must_use]
pub fn dummy<T: Field>() -> Ops<T> { Ops::default() }

columns_view_impl!(Register);
make_col_map!(Register);
/// [`Design doc for RegisterSTARK`](https://www.notion.so/0xmozak/Register-File-STARK-62459d68aea648a0abf4e97aa0093ea2?pvs=4#0729f89ddc724967ac991c9e299cc4fc)
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct Register<T> {
    /// The register 'address' that indexes into 1 of our 32 registers.
    /// Should only take values 0-31, so this column should be a running sum
    /// from 0 to 31 (inclusive). Note that this isn't the same as memory
    /// address.
    pub addr: T,

    /// Value of the register at time (in clk) of access.
    pub value: T,

    /// Augmented clock at register access. This is calculated as:
    /// `augmented_clk` = clk * 2 for register reads, and
    /// `augmented_clk` = clk * 2 + 1 for register writes,
    /// to ensure that we do not write to the register before we read.
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

columns_view_impl!(RegisterCtl);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct RegisterCtl<T> {
    pub clk: T,
    pub op: T,
    pub addr: T,
    pub value: T,
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

#[cfg(feature = "enable_register_starks")]
#[must_use]
pub fn lookup_for_register_init() -> TableWithTypedOutput<RegisterInitCtl<Column>> {
    let reg = COL_MAP;
    RegisterTable::new(
        RegisterInitCtl {
            addr: reg.addr,
            value: reg.value,
        },
        reg.ops.is_init,
    )
}

#[cfg(feature = "enable_register_starks")]
#[must_use]
pub fn register_looked() -> TableWithTypedOutput<RegisterCtl<Column>> {
    // use crate::linear_combination_typed::ColumnWithTypedInput;

    // let reg: Register<Column> = col_map().map(Column::from);
    // let reg = COL_MAP;
    // let ops = COL_MAP.ops;
    todo!()
    // RegisterTable::new(
    //     vec![
    //         ColumnWithTypedInput::ascending_sum(ops),
    //         reg.clk,
    //         reg.addr,
    //         reg.value,
    //     ],
    //     ops.is_read + ops.is_write,
    // )
}

#[cfg(feature = "enable_register_starks")]
#[must_use]
pub fn rangecheck_looking() -> Vec<TableWithTypedOutput<RangeCheckCtl<Column>>> {
    use crate::linear_combination_typed::ColumnWithTypedInput;

    let lv = COL_MAP;
    let nv = COL_MAP.map(ColumnWithTypedInput::flip);
    let new = RangeCheckCtl::new;
    vec![RegisterTable::new(
        new(nv.augmented_clk() - lv.augmented_clk()),
        nv.is_rw(),
    )]
}

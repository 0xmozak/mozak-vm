use core::ops::Add;

use plonky2::field::types::Field;

use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};
use crate::cross_table_lookup::Column;
// use crate::stark::mozak_stark::{HalfWordMemoryTable, Table};

// TODO(roman): address_limbs & value columns can be optimized
// value == linear combination via range-check
// address_limbs also linear combination + forbid  wrapping add
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct HalfWordMemory<T> {
    /// Clock at memory access.
    pub clk: T,
    /// Memory address.
    pub addr: T,
    // Operations (one-hot encoded)
    // One of `is_sh`, `is_lhu`
    // If none are `1`, it is a padding row
    /// Binary filter column to represent a RISC-V SH operation.
    pub is_sh: T,
    /// Binary filter column to represent a RISC-V LHU operation.
    pub is_lhu: T,
    /// Address of LSB byte
    pub addr_limb1: T,
    /// LSB byte
    pub limb0: T,
    /// MSB byte
    pub limb1: T,
    // dummy - TODO: remove it once rust-generics problem are solved
    pub dummy: [T; 17],
}

columns_view_impl!(HalfWordMemory);
make_col_map!(HalfWordMemory);

impl<T: Clone + Add<Output = T>> HalfWordMemory<T> {
    pub fn is_executed(&self) -> T {
        let s: HalfWordMemory<T> = self.clone();
        s.is_sh + s.is_lhu
    }
}

/// Total number of columns.
pub const NUM_HW_MEM_COLS: usize = HalfWordMemory::<()>::NUMBER_OF_COLUMNS;

/// Columns containing the data which are looked from the CPU table into Memory
/// stark table.
#[must_use]
pub fn data_for_cpu<F: Field>() -> Vec<Column<F>> {
    let mem = MAP.map(Column::from);
    vec![
        mem.clk,
        mem.addr,
        // Column::reduce_with_powers(vec![mem.limb0, mem.limb1], 1 << 8),
        mem.limb0 + mem.limb1 * F::from_canonical_u32(1 << 8),
        mem.is_lhu,
        mem.is_sh,
    ]
}

/// Columns containing the data which are looked from the halfword memory table
/// into Memory stark table.
#[must_use]
pub fn data_for_memory_limb0<F: Field>() -> Vec<Column<F>> {
    let mem = MAP.map(Column::from);
    vec![mem.clk, mem.addr, mem.limb0, mem.is_lhu, mem.is_sh]
}

/// Columns containing the data which are looked from the halfword memory table
/// into Memory stark table.
#[must_use]
pub fn data_for_memory_limb1<F: Field>() -> Vec<Column<F>> {
    let mem = MAP.map(Column::from);
    vec![
        mem.clk,
        mem.addr,
        mem.limb1,
        mem.is_lhu,
        mem.is_sh,
        Column::constant(F::ONE), // is_init
    ]
}

/// Column for a binary filter to indicate a lookup from the CPU table into
/// Memory stark table.
#[must_use]
pub fn filter_for_cpu<F: Field>() -> Column<F> {
    let mem = MAP.map(Column::from);
    mem.is_sh + mem.is_lhu
}
/// Column for a binary filter to indicate a lookup from the CPU table into
/// Memory stark table.
#[must_use]
pub fn filter_for_memory<F: Field>() -> Column<F> {
    let mem = MAP.map(Column::from);
    mem.is_sh + mem.is_lhu
}

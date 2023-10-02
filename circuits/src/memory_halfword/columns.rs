use core::ops::Add;

use plonky2::field::types::Field;

use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};
use crate::cross_table_lookup::Column;
// use crate::stark::mozak_stark::{HalfWordMemoryTable, Table};

/// Operations (one-hot encoded)
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct Ops<T> {
    // One of `is_sh`, `is_lhu`
    // If none are `1`, it is a padding row
    /// Binary filter column to represent a RISC-V SH operation.
    pub is_sh: T,
    /// Binary filter column to represent a RISC-V LHU operation.
    pub is_lhu: T,
}

// TODO(roman): address_limbs & value columns can be optimized
// value == linear combination via range-check
// address_limbs also linear combination + forbid  wrapping add
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct HalfWordMemory<T> {
    /// Clock at memory access.
    pub clk: T,
    pub ops: Ops<T>,
    /// Memory addresses for the one byte limbs
    pub addrs: [T; 2],
    pub limbs: [T; 2],
}

columns_view_impl!(HalfWordMemory);
make_col_map!(HalfWordMemory);

impl<T: Clone + Add<Output = T>> HalfWordMemory<T> {
    pub fn is_executed(&self) -> T {
        let ops: Ops<T> = self.ops.clone();
        ops.is_lhu + ops.is_sh
    }
}

/// Total number of columns.
pub const NUM_HW_MEM_COLS: usize = HalfWordMemory::<()>::NUMBER_OF_COLUMNS;

// /// TBD - each byte is range-checked in byte-memory table so, maybe avoided
// // #[must_use]
// pub fn rangecheck_looking<F: Field>() -> Vec<Table<F>> {
//     let mem = MAP.map(Column::from);
//     vec![
//         HalfWordMemoryTable::new(Column::singles([MAP.limb0]),
// mem.is_executed()),         HalfWordMemoryTable::new(Column::singles([MAP.
// limb1]), mem.is_executed()),     ]
// }

/// Columns containing the data which are looked from the CPU table into Memory
/// stark table.
#[must_use]
pub fn data_for_cpu<F: Field>() -> Vec<Column<F>> {
    let mem = MAP.map(Column::from);
    vec![
        mem.clk,
        mem.addrs[0].clone(),
        Column::reduce_with_powers(&mem.limbs, F::from_canonical_u16(1 << 8)),
        mem.ops.is_lhu,
        mem.ops.is_sh,
    ]
}

/// Columns containing the data which are looked from the halfword memory table
/// into Memory stark table.
#[must_use]
pub fn data_for_memory_limb0<F: Field>() -> Vec<Column<F>> {
    let mem = MAP.map(Column::from);
    vec![
        mem.clk,
        mem.addrs[0].clone(),
        mem.limbs[0].clone(),
        mem.ops.is_lhu,
        mem.ops.is_sh,
    ]
}

/// Columns containing the data which are looked from the halfword memory table
/// into Memory stark table.
#[must_use]
pub fn data_for_memory_limb1<F: Field>() -> Vec<Column<F>> {
    let mem = MAP.map(Column::from);
    vec![
        mem.clk,
        mem.addrs[1].clone(),
        mem.limbs[1].clone(),
        mem.ops.is_lhu,
        mem.ops.is_sh,
        // TODO: Roman - add is_init constant
    ]
}

/// Column for a binary filter to indicate a lookup
#[must_use]
pub fn filter<F: Field>() -> Column<F> {
    let ops = MAP.map(Column::from).ops;
    ops.is_sh + ops.is_lhu
}

use core::ops::Add;

use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};
use crate::cross_table_lookup::Column;
use crate::stark::mozak_stark::{HalfWordMemoryTable, Table};
// use crate::stark::mozak_stark::{HalfWordMemoryTable, Table};

/// Operations (one-hot encoded)
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct Ops<T> {
    // One of `is_store`, `is_load_u`
    // If none are `1`, it is a padding row
    /// Binary filter column to represent a RISC-V SH operation.
    pub is_store: T,
    /// Binary filter column to represent a RISC-V LHU operation.
    pub is_load: T,
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
        ops.is_load + ops.is_store
    }
}

/// Total number of columns.
pub const NUM_HW_MEM_COLS: usize = HalfWordMemory::<()>::NUMBER_OF_COLUMNS;

/// Lookup from CPU table into halfword memory table.
#[must_use]
pub fn lookup_for_cpu() -> Table {
    let mem = col_map().map(Column::from);
    HalfWordMemoryTable::new(
        vec![
            mem.clk,
            mem.addrs[0].clone(),
            Column::reduce_with_powers(&mem.limbs, 1 << 8),
            mem.ops.is_store,
            mem.ops.is_load,
        ],
        col_map().map(Column::from).is_executed(),
    )
}

/// Lookup from halfword memory table into Memory stark table.
#[must_use]
pub fn lookup_for_memory_limb(limb_index: usize) -> Table {
    assert!(
        limb_index < 2,
        "limb_index is {limb_index} but it should be in 0..2 range"
    );
    let mem = col_map().map(Column::from);
    HalfWordMemoryTable::new(
        vec![
            mem.clk,
            mem.ops.is_store,
            mem.ops.is_load,
            mem.limbs[limb_index].clone(),
            mem.addrs[limb_index].clone(),
        ],
        col_map().map(Column::from).is_executed(),
    )
}

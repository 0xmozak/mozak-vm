use core::ops::Add;

use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};
use crate::cross_table_lookup::ColumnX;
use crate::linear_combination::Column;
use crate::memory::columns::MemoryCtl;
use crate::stark::mozak_stark::{HalfWordMemoryTable, TableNamed};
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

make_col_map!(HalfWordMemory);
columns_view_impl!(HalfWordMemory);
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

impl<T: Clone + Add<Output = T>> HalfWordMemory<T> {
    pub fn is_executed(&self) -> T {
        let ops: Ops<T> = self.ops.clone();
        ops.is_load + ops.is_store
    }
}

/// Total number of columns.
pub const NUM_HW_MEM_COLS: usize = HalfWordMemory::<()>::NUMBER_OF_COLUMNS;

/// Columns containing the data which are looked from the CPU table into Memory
/// stark table.
#[must_use]
pub fn lookup_for_cpu() -> TableNamed<MemoryCtl<Column>> {
    let mem = COL_MAP;
    HalfWordMemoryTable::new(
        MemoryCtl {
            clk: mem.clk,
            is_store: mem.ops.is_store,
            is_load: mem.ops.is_load,
            value: ColumnX::reduce_with_powers(mem.limbs, 1 << 8),
            addr: mem.addrs[0],
        },
        COL_MAP.is_executed(),
    )
}

/// Lookup into Memory stark table.
#[must_use]
pub fn lookup_for_memory_limb(limb_index: usize) -> TableNamed<MemoryCtl<Column>> {
    assert!(
        limb_index < 2,
        "limb_index is {limb_index} but it should be in 0..2 range"
    );
    let mem = COL_MAP;
    HalfWordMemoryTable::new(
        MemoryCtl {
            clk: mem.clk,
            is_store: mem.ops.is_store,
            is_load: mem.ops.is_load,
            value: mem.limbs[limb_index],
            addr: mem.addrs[limb_index],
        },
        COL_MAP.is_executed(),
    )
}

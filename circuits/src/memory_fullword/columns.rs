use core::ops::Add;

use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};
use crate::cross_table_lookup::ColumnTyped;
use crate::linear_combination::Column;
use crate::memory::columns::MemoryCtl;
use crate::stark::mozak_stark::{FullWordMemoryTable, TableWithUntypedInput};

/// Operations (one-hot encoded)
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct Ops<T> {
    // One of `is_store`, `is_load`
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
pub struct FullWordMemory<T> {
    /// Clock at memory access.
    pub clk: T,
    pub ops: Ops<T>,
    /// Memory addresses for the one byte limbs
    pub addrs: [T; 4],
    pub limbs: [T; 4],
}

columns_view_impl!(FullWordMemory);
make_col_map!(FullWordMemory);

impl<T: Copy + Add<Output = T>> FullWordMemory<T> {
    pub fn is_executed(&self) -> T {
        let ops = self.ops;
        ops.is_load + ops.is_store
    }
}

/// Total number of columns.
pub const NUM_HW_MEM_COLS: usize = FullWordMemory::<()>::NUMBER_OF_COLUMNS;

/// Columns containing the data which are looked from the CPU table into Memory
/// stark table.
#[must_use]
pub fn lookup_for_cpu() -> TableWithUntypedInput<MemoryCtl<Column>> {
    let mem = COL_MAP;
    FullWordMemoryTable::new(
        MemoryCtl {
            clk: mem.clk,
            is_store: mem.ops.is_store,
            is_load: mem.ops.is_load,
            value: ColumnTyped::reduce_with_powers(mem.limbs, 1 << 8),
            addr: mem.addrs[0],
        },
        COL_MAP.is_executed(),
    )
}

/// Lookup between fullword memory table
/// and Memory stark table.
#[must_use]
pub fn lookup_for_memory_limb(limb_index: usize) -> TableWithUntypedInput<MemoryCtl<Column>> {
    assert!(limb_index < 4, "limb-index can be 0..4");
    let mem = COL_MAP;
    FullWordMemoryTable::new(
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

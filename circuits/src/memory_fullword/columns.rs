use core::ops::Add;

use plonky2::field::types::Field;

use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};
use crate::cross_table_lookup::Column;

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

impl<T: Clone + Add<Output = T>> FullWordMemory<T> {
    pub fn is_executed(&self) -> T {
        let ops: Ops<T> = self.ops.clone();
        ops.is_load + ops.is_store
    }
}

/// Total number of columns.
pub const NUM_HW_MEM_COLS: usize = FullWordMemory::<()>::NUMBER_OF_COLUMNS;

/// Columns containing the data which are looked from the CPU table into Memory
/// stark table.
#[must_use]
pub fn data_for_cpu<F: Field>() -> Vec<Column<F>> {
    let mem = col_map().map(Column::from);
    vec![
        mem.clk,
        mem.addrs[0].clone(),
        Column::reduce_with_powers(&mem.limbs, F::from_canonical_u16(1 << 8)),
        mem.ops.is_store,
        mem.ops.is_load,
    ]
}

/// Columns containing the data which are looked from the fullword memory table
/// into Memory stark table.
#[must_use]
pub fn data_for_memory_limb<F: Field>(limb_index: usize) -> Vec<Column<F>> {
    assert!(limb_index < 4, "limb-index can be 0..4");
    let mem = col_map().map(Column::from);
    vec![
        mem.clk,
        mem.ops.is_store,
        mem.ops.is_load,
        mem.limbs[limb_index].clone(),
        mem.addrs[limb_index].clone(),
    ]
}
/// Column for a binary filter to indicate a lookup
#[must_use]
pub fn filter<F: Field>() -> Column<F> { col_map().map(Column::from).is_executed() }

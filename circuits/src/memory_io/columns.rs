use core::ops::Add;

use plonky2::field::types::Field;

use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};
use crate::cross_table_lookup::Column;
// use crate::stark::mozak_stark::{HalfWordMemoryTable, Table};

/// Operations (one-hot encoded)
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct Ops<T> {
    // One of `is_store`, `is_load_u`
    // If none are `1`, it is a padding row
    /// Binary filter column to represent a RISC-V SH operation.
    pub is_memory_store: T,
    /// Binary filter column to represent a RISC-V LHU operation.
    pub is_memory_load: T,
    /// Binary filter column to represent a io-write operation.
    pub is_io_store: T,
    /// Binary filter column to represent a io-read operation.
    pub is_io_load: T,
}

#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct InputOutputMemory<T> {
    /// Clock at memory access.
    pub clk: T,
    pub ops: Ops<T>,
    /// Address: start-address
    pub address: T,
    /// Size: size of io-chunk in bytes
    pub size: T,
    /// Value: byte value
    pub value: T,
}

columns_view_impl!(InputOutputMemory);
make_col_map!(InputOutputMemory);

impl<T: Clone + Add<Output = T>> InputOutputMemory<T> {
    pub fn is_io(&self) -> T {
        let ops: Ops<T> = self.ops.clone();
        ops.is_io_load + ops.is_io_store
    }

    pub fn is_memory(&self) -> T {
        let ops: Ops<T> = self.ops.clone();
        ops.is_memory_load + ops.is_memory_store
    }

    pub fn is_executed(&self) -> T {
        let ops: Ops<T> = self.ops.clone();
        ops.is_io_load + ops.is_io_store + ops.is_memory_load + ops.is_memory_store
    }
}

/// Total number of columns.
pub const NUM_HW_MEM_COLS: usize = InputOutputMemory::<()>::NUMBER_OF_COLUMNS;

/// Columns containing the data which are looked from the CPU table into Memory
/// stark table.
#[must_use]
pub fn data_for_cpu<F: Field>() -> Vec<Column<F>> {
    let mem = MAP.map(Column::from);
    vec![
        mem.clk,
        mem.address,
        mem.size,
        mem.ops.is_io_store,
        mem.ops.is_io_load,
    ]
}

/// Column for a binary filter to indicate a lookup
#[must_use]
pub fn filter_for_cpu<F: Field>() -> Column<F> { MAP.map(Column::from).is_io() }

/// Columns containing the data which are looked from the halfword memory table
/// into Memory stark table.
#[must_use]
pub fn data_for_memory<F: Field>() -> Vec<Column<F>> {
    let mem = MAP.map(Column::from);
    vec![
        mem.clk,
        mem.ops.is_memory_store,
        mem.ops.is_memory_load,
        mem.value,
        mem.address,
    ]
}

/// Column for a binary filter to indicate a lookup
#[must_use]
pub fn filter_for_memory<F: Field>() -> Column<F> { MAP.map(Column::from).is_memory() }

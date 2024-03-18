use core::ops::Add;

use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};
use crate::cross_table_lookup::ColumnX;
use crate::linear_combination::Column;
use crate::memory::columns::MemoryCtl;
use crate::stark::mozak_stark::{TableKind, TableNamed};

/// Operations (one-hot encoded)
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct Ops<T> {
    /// Binary filter column to represent a RISC-V SB operation.
    pub is_memory_store: T,
    /// Binary filter column to represent a io-read operation.
    pub is_io_store: T,
}

#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct InputOutputMemory<T> {
    /// Clock at memory access.
    pub clk: T,
    /// Address: start-address
    pub addr: T,
    /// Size: size of io-chunk in bytes
    pub size: T,
    /// Value: byte value
    pub value: T,
    /// Operation: io_store/load io_memory_store/load
    pub ops: Ops<T>,
    /// Helper to decrease poly degree
    pub is_lv_and_nv_are_memory_rows: T,
}

columns_view_impl!(InputOutputMemory);
make_col_map!(InputOutputMemory);

impl<T: Clone + Add<Output = T>> InputOutputMemory<T> {
    pub fn is_executed(&self) -> T {
        self.ops.is_io_store.clone() + self.ops.is_memory_store.clone()
    }
}

/// Total number of columns.
pub const NUM_IO_MEM_COLS: usize = InputOutputMemory::<()>::NUMBER_OF_COLUMNS;

columns_view_impl!(InputOutputMemoryCtl);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct InputOutputMemoryCtl<T> {
    pub clk: T,
    pub addr: T,
    pub size: T,
}

/// Lookup between CPU table and Memory stark table.
#[must_use]
pub fn lookup_for_cpu(kind: TableKind) -> TableNamed<InputOutputMemoryCtl<Column>> {
    let mem = COL_MAP;
    TableNamed {
        kind,
        columns: InputOutputMemoryCtl {
            clk: mem.clk,
            addr: mem.addr,
            size: mem.size,
        }
        .into_iter()
        .map(Column::from)
        .collect(),
        filter_column: COL_MAP.ops.is_io_store.into(),
    }
}

/// Lookup into Memory stark table.
#[must_use]
pub fn lookup_for_memory(kind: TableKind) -> TableNamed<MemoryCtl<Column>> {
    let mem = COL_MAP;

    TableNamed {
        kind,
        columns: MemoryCtl {
            clk: mem.clk,
            is_store: mem.ops.is_memory_store,
            is_load: ColumnX::constant(0),
            value: mem.value,
            addr: mem.addr,
        }
        .into_iter()
        .map(Column::from)
        .collect(),
        filter_column: COL_MAP.ops.is_memory_store.into(),
    }
}

use core::ops::Add;

use mozak_sdk::core::reg_abi::REG_A1;

use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};
use crate::cross_table_lookup::{Column, ColumnWithTypedInput};
use crate::memory::columns::MemoryCtl;
use crate::register::columns::RegisterCtl;
use crate::stark::mozak_stark::{
    IoMemoryPrivateTable, IoMemoryPublicTable, IoTranscriptTable, TableKind, TableWithTypedOutput,
};

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
    /// Operation: `io_store/load` `io_memory_store/load`
    pub ops: Ops<T>,
    /// Helper to decrease poly degree
    pub is_lv_and_nv_are_memory_rows: T,
}

columns_view_impl!(InputOutputMemory);
make_col_map!(InputOutputMemory);

impl<T: Copy + Add<Output = T>> InputOutputMemory<T> {
    pub fn is_executed(&self) -> T { self.ops.is_io_store + self.ops.is_memory_store }
}

/// Total number of columns.
pub const NUM_IO_MEM_COLS: usize = InputOutputMemory::<()>::NUMBER_OF_COLUMNS;

columns_view_impl!(InputOutputMemoryCtl);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct InputOutputMemoryCtl<T> {
    pub op: T,
    pub clk: T,
    pub addr: T,
    pub size: T,
}

/// Lookup between CPU table and Memory stark table.
#[must_use]
pub fn lookup_for_cpu(
    kind: TableKind,
    op: i64,
) -> TableWithTypedOutput<InputOutputMemoryCtl<Column>> {
    TableWithTypedOutput {
        kind,
        columns: InputOutputMemoryCtl {
            op: ColumnWithTypedInput::constant(op),
            clk: COL_MAP.clk,
            addr: COL_MAP.addr,
            size: COL_MAP.size,
        }
        .into_iter()
        .map(Column::from)
        .collect(),
        filter_column: COL_MAP.ops.is_io_store.into(),
    }
}

/// Lookup into Memory stark table.
#[must_use]
pub fn lookup_for_memory(kind: TableKind) -> TableWithTypedOutput<MemoryCtl<Column>> {
    TableWithTypedOutput {
        kind,
        columns: MemoryCtl {
            clk: COL_MAP.clk,
            is_store: COL_MAP.ops.is_memory_store,
            is_load: ColumnWithTypedInput::constant(0),
            value: COL_MAP.value,
            addr: COL_MAP.addr,
        }
        .into_iter()
        .map(Column::from)
        .collect(),
        filter_column: COL_MAP.ops.is_memory_store.into(),
    }
}

#[must_use]
pub fn register_looking() -> Vec<TableWithTypedOutput<RegisterCtl<Column>>> {
    let mem = COL_MAP;
    let data = RegisterCtl {
        clk: mem.clk,
        op: ColumnWithTypedInput::constant(1), // read
        addr: ColumnWithTypedInput::constant(i64::from(REG_A1)),
        value: mem.addr,
    };
    vec![
        IoMemoryPrivateTable::new(data, mem.ops.is_io_store),
        IoMemoryPublicTable::new(data, mem.ops.is_io_store),
        IoTranscriptTable::new(data, mem.ops.is_io_store),
    ]
}

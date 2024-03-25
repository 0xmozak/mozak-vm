use core::ops::Add;

#[cfg(feature = "enable_register_starks")]
use mozak_sdk::core::reg_abi::REG_A1;

use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};
use crate::cross_table_lookup::{Column, ColumnWithTypedInput};
use crate::memory::columns::MemoryCtl;
#[cfg(feature = "enable_register_starks")]
use crate::register::columns::RegisterCtl;
#[cfg(feature = "enable_register_starks")]
use crate::stark::mozak_stark::{IoMemoryPrivateTable, IoMemoryPublicTable, IoTranscriptTable};
use crate::stark::mozak_stark::{TableKind, TableWithTypedOutput};

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
    pub clk: T,
    pub addr: T,
    pub size: T,
}

/// Lookup between CPU table and Memory stark table.
#[must_use]
pub fn lookup_for_cpu(kind: TableKind) -> TableWithTypedOutput<InputOutputMemoryCtl<Column>> {
    let mem = COL_MAP;
    TableWithTypedOutput {
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
pub fn lookup_for_memory(kind: TableKind) -> TableWithTypedOutput<MemoryCtl<Column>> {
    let mem = COL_MAP;

    TableWithTypedOutput {
        kind,
        columns: MemoryCtl {
            clk: mem.clk,
            is_store: mem.ops.is_memory_store,
            is_load: ColumnWithTypedInput::constant(0),
            value: mem.value,
            addr: mem.addr,
        }
        .into_iter()
        .map(Column::from)
        .collect(),
        filter_column: COL_MAP.ops.is_memory_store.into(),
    }
}

// Look up a read into register table with:
//
// read at augmented clk
//
// REG_A0 -> public input to ecall type (private or public io read, via)
// (Actually, can be hard-coded from the point of view of the proof; doesn't
// need to be PUBLIC_INPUT read REG_A1 -> addr
// read REG_A2 -> size
//
// filter = is_memory_store
/// TODO: at the moment weonly do addr; look up the rest, too.  Adjust trace
/// generation.
/// TODO: write a mechanism that generates register-read-traces automatically
/// from the CTL data.  Similar to what we did for generating range-check traces
/// automatically.
#[cfg(feature = "enable_register_starks")]
#[must_use]
pub fn register_looking() -> Vec<TableWithTypedOutput<RegisterCtl<Column>>> {
    let mem = COL_MAP;
    let data = RegisterCtl {
        clk: mem.clk,
        // read
        op: ColumnWithTypedInput::constant(1),
        addr: ColumnWithTypedInput::constant(i64::from(REG_A1)),
        value: mem.addr,
    };
    vec![
        IoMemoryPrivateTable::new(data, mem.ops.is_io_store),
        IoMemoryPublicTable::new(data, mem.ops.is_io_store),
        IoTranscriptTable::new(data, mem.ops.is_io_store),
    ]
}

use core::ops::Add;

use mozak_sdk::core::ecall::COMMITMENT_SIZE;
use mozak_sdk::core::reg_abi::REG_A1;

use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};
use crate::cross_table_lookup::{Column, ColumnWithTypedInput};
use crate::memory::columns::MemoryCtl;
use crate::register::RegisterCtl;
use crate::stark::mozak_stark::{
    CallTapeTable, CastListCommitmentTapeTable, EventsCommitmentTapeTable,
    StorageDevicePrivateTable, StorageDevicePublicTable, TableKind, TableWithTypedOutput,
};
use crate::tape_commitments::columns::TapeCommitmentCTL;

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
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct StorageDevice<T> {
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

columns_view_impl!(StorageDevice);
make_col_map!(StorageDevice);

impl<T: Copy + Add<Output = T>> StorageDevice<T> {
    pub fn is_executed(&self) -> T { self.ops.is_io_store + self.ops.is_memory_store }
}

/// Total number of columns.
pub const NUM_IO_MEM_COLS: usize = StorageDevice::<()>::NUMBER_OF_COLUMNS;

columns_view_impl!(StorageDeviceCtl);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct StorageDeviceCtl<T> {
    pub op: T,
    pub clk: T,
    pub addr: T,
    pub size: T,
}

/// Lookup between CPU table and Memory stark table.
#[must_use]
pub fn lookup_for_cpu(kind: TableKind, op: i64) -> TableWithTypedOutput<StorageDeviceCtl<Column>> {
    TableWithTypedOutput {
        kind,
        columns: StorageDeviceCtl {
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
    let data = RegisterCtl {
        clk: COL_MAP.clk,
        op: ColumnWithTypedInput::constant(1), // read
        addr: ColumnWithTypedInput::constant(i64::from(REG_A1)),
        value: COL_MAP.addr,
    };
    vec![
        StorageDevicePrivateTable::new(data, COL_MAP.ops.is_io_store),
        StorageDevicePublicTable::new(data, COL_MAP.ops.is_io_store),
        CallTapeTable::new(data, COL_MAP.ops.is_io_store),
        EventsCommitmentTapeTable::new(data, COL_MAP.ops.is_io_store),
        CastListCommitmentTapeTable::new(data, COL_MAP.ops.is_io_store),
    ]
}

#[must_use]
pub fn event_commitment_lookup_in_tape_commitments(
) -> TableWithTypedOutput<TapeCommitmentCTL<Column>> {
    let data = TapeCommitmentCTL {
        byte: COL_MAP.value,
        index: i64::try_from(COMMITMENT_SIZE - 1).unwrap() - COL_MAP.size,
    };
    EventsCommitmentTapeTable::new(data, COL_MAP.ops.is_memory_store)
}

#[must_use]
pub fn castlist_commitment_lookup_in_tape_commitments(
) -> TableWithTypedOutput<TapeCommitmentCTL<Column>> {
    let data = TapeCommitmentCTL {
        byte: COL_MAP.value,
        index: i64::try_from(COMMITMENT_SIZE - 1).unwrap() - COL_MAP.size,
    };
    CastListCommitmentTapeTable::new(data, COL_MAP.ops.is_memory_store)
}

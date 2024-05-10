use core::ops::Add;

use mozak_runner::reg_abi::REG_A1;
use sdk_core_types::constants::poseidon2::DIGEST_BYTES;

use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};
use crate::cross_table_lookup::{Column, ColumnWithTypedInput};
use crate::memory::columns::MemoryCtl;
use crate::register::RegisterCtl;
use crate::stark::mozak_stark::{
    CallTapeTable, CastListCommitmentTapeTable, EventsCommitmentTapeTable, SelfProgIdTapeTable,
    StorageDevicePrivateTable, StorageDevicePublicTable, TableKind, TableWithTypedOutput,
};
use crate::tape_commitments::columns::TapeCommitmentCTL;

/// Operations (one-hot encoded)
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct Ops<T> {
    /// Binary filter column to represent a RISC-V SB operation.
    pub is_memory_store: T,
    /// Binary filter column to represent an storage device operation.
    pub is_storage_device: T,
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
    /// Operation: one-hot encoded
    pub ops: Ops<T>,
    /// Helper to decrease poly degree
    pub is_lv_and_nv_are_memory_rows: T,
}

columns_view_impl!(StorageDevice);
make_col_map!(StorageDevice);

impl<T: Copy + Add<Output = T>> StorageDevice<T> {
    pub fn is_executed(&self) -> T { self.ops.is_storage_device + self.ops.is_memory_store }
}

/// Total number of columns.
pub const NUM_STORAGE_DEVICE_COLS: usize = StorageDevice::<()>::NUMBER_OF_COLUMNS;

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
        filter_column: COL_MAP.ops.is_storage_device.into(),
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
        StorageDevicePrivateTable::new(data, COL_MAP.ops.is_storage_device),
        StorageDevicePublicTable::new(data, COL_MAP.ops.is_storage_device),
        CallTapeTable::new(data, COL_MAP.ops.is_storage_device),
        EventsCommitmentTapeTable::new(data, COL_MAP.ops.is_storage_device),
        CastListCommitmentTapeTable::new(data, COL_MAP.ops.is_storage_device),
        SelfProgIdTapeTable::new(data, COL_MAP.ops.is_storage_device),
    ]
}

#[must_use]
pub fn event_commitment_lookup_in_tape_commitments(
) -> TableWithTypedOutput<TapeCommitmentCTL<Column>> {
    let data = TapeCommitmentCTL {
        byte: COL_MAP.value,
        index: i64::try_from(DIGEST_BYTES - 1).unwrap() - COL_MAP.size,
    };
    EventsCommitmentTapeTable::new(data, COL_MAP.ops.is_memory_store)
}

#[must_use]
pub fn castlist_commitment_lookup_in_tape_commitments(
) -> TableWithTypedOutput<TapeCommitmentCTL<Column>> {
    let data = TapeCommitmentCTL {
        byte: COL_MAP.value,
        index: i64::try_from(DIGEST_BYTES - 1).unwrap() - COL_MAP.size,
    };
    CastListCommitmentTapeTable::new(data, COL_MAP.ops.is_memory_store)
}

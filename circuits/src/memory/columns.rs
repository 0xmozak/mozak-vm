use plonky2::field::types::Field;

use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};
use crate::cross_table_lookup::Column;
use crate::stark::mozak_stark::{MemoryTable, Table};

#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct Memory<T> {
    /// Indicates if a the memory address is writable.
    pub is_writable: T,

    /// Memory address.
    pub addr: T,

    // Clock at memory access.
    pub clk: T,

    /// Operations (one-hot encoded)
    /// One of `is_sb`, `is_lbu` or `is_init`(static meminit from ELF) == 1.
    /// If none are `1`, it is a padding row
    pub is_sb: T, // RISC-V Operation: SB
    pub is_lbu: T,  // RISC-V Operation: LBU
    pub is_init: T, // Memory Initialisation from ELF (prior to vm execution)

    /// Value of memory access.
    pub value: T,

    /// Difference between current and previous address.
    pub diff_addr: T,

    /// Inverse of the above column. 0 if the `diff_addr` is 0.
    pub diff_addr_inv: T,

    /// Difference between current and previous clock.
    pub diff_clk: T,
}
columns_view_impl!(Memory);
make_col_map!(Memory);

/// Total number of columns.
pub const NUM_MEM_COLS: usize = Memory::<()>::NUMBER_OF_COLUMNS;

#[must_use]
pub fn rangecheck_looking<F: Field>() -> Vec<Table<F>> {
    let mem = MAP.map(Column::from);
    let is_executed = mem.is_sb + mem.is_lbu + mem.is_init;
    vec![
        MemoryTable::new(Column::singles([MAP.diff_addr]), is_executed.clone()),
        MemoryTable::new(Column::singles([MAP.diff_clk]), is_executed),
    ]
}

/// Columns containing the data which are looked from the CPU table into Memory
/// stark table.
#[must_use]
pub fn data_for_cpu<F: Field>() -> Vec<Column<F>> {
    vec![
        Column::single(MAP.clk),
        // TODO(Supragya): Add CTL for Opcodes SB and LBU, requires memory table
        // to have one-hot OP encoding
        Column::single(MAP.value),
    ]
}

/// Column for a binary filter to indicate a lookup from the CPU table into
/// Memory stark table.
#[must_use]
pub fn filter_for_cpu<F: Field>() -> Column<F> {
    let mem = MAP.map(Column::from);
    mem.is_sb + mem.is_lbu + mem.is_init
}

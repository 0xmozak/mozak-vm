use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};
use crate::cross_table_lookup::Column;

columns_view_impl!(InstructionRow);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct InstructionRow<T> {
    // Design doc for CPU <> Program cross-table-lookup:
    // https://www.notion.so/0xmozak/Cross-Table-Lookup-bbe98d9471114c36a278f0c491f203e5#c3876d13c1f94b7ab154ea1f8b908181
    pub pc: T,
    /// inst_data include:
    /// - ops: This is an internal opcode, not the opcode from RISC-V
    /// - is_op1_signed and is_op2_signed
    /// - rs1_select, rs2_select, and rd_select
    /// - imm_value
    pub inst_data: T,
}

columns_view_impl!(ProgramRom);
make_col_map!(ProgramRom);
/// A Row of ROM generated from read-only memory
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct ProgramRom<T> {
    pub inst: InstructionRow<T>,
    /// Filters out instructions that are duplicates, i.e., appear more than
    /// once in the trace.
    pub filter: T,
}

// Total number of columns.
pub const NUM_PROGRAM_COLS: usize = ProgramRom::<()>::NUMBER_OF_COLUMNS;

#[must_use]
pub fn data_for_ctl() -> InstructionRow<Column> { col_map().inst.map(Column::from) }

use plonky2::field::types::Field;

use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};
use crate::cross_table_lookup::Column;

columns_view_impl!(InstructionRow);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct InstructionRow<T> {
    // Design doc for CPU <> Program cross-table-lookup:
    // https://www.notion.so/0xmozak/Cross-Table-Lookup-bbe98d9471114c36a278f0c491f203e5#c3876d13c1f94b7ab154ea1f8b908181
    pub pc: T,
    /// internal opcode, not the opcode from RISC-V
    pub opcode: T,
    pub rs1: T,
    pub rs2: T,
    pub rd: T,
    pub imm: T,
}

columns_view_impl!(ProgramRom);
make_col_map!(ProgramRom);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct ProgramRom<T> {
    pub inst: InstructionRow<T>,
    /// Filters out instructions that are duplicates, i.e., appear more than
    /// once in the trace.
    pub filter: T,
    /// TODO(Supragya): Phantom column to make the [(); X]: thingy work.
    /// No practical need otherwise, remove when possible.
    pub phantomcol: T,
}

// Total number of columns.
pub const NUM_PROGRAM_COLS: usize = ProgramRom::<()>::NUMBER_OF_COLUMNS;

#[must_use]
pub fn data_for_ctl<F: Field>() -> Vec<Column<F>> { Column::singles(MAP.inst) }

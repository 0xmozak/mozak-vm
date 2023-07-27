use itertools::Itertools;
use plonky2::field::types::Field;

use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};
use crate::cross_table_lookup::Column;

#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct ProgramColumnsView<T: Copy> {
    pub program_is_inst: T,
    pub program_pc: T,
    pub program_opcode: T,
    pub program_rs1: T,
    pub program_rs2: T,
    pub program_rd: T,
    pub program_imm: T,
}
columns_view_impl!(ProgramColumnsView);
make_col_map!(ProgramColumnsView);

// Total number of columns.
pub const NUM_PROGRAM_COLS: usize = ProgramColumnsView::<()>::NUMBER_OF_COLUMNS;

#[must_use]
pub fn data_for_ctl<F: Field>() -> Vec<Column<F>> {
    Column::singles([
        MAP.program_pc,
        MAP.program_opcode,
        MAP.program_rs1,
        MAP.program_rs2,
        MAP.program_rd,
        MAP.program_imm,
    ])
    .collect_vec()
}

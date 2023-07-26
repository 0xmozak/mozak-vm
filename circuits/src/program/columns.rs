use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};

#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct ProgramColumnsView<T: Copy> {
    pub program_is_inst: T,
    pub program_pc: T,
    pub program_inst: T,
    pub program_rs1: T,
    pub program_rs2: T,
    pub program_rd: T,
    pub program_imm: T,
}
columns_view_impl!(ProgramColumnsView);
make_col_map!(ProgramColumnsView);

// Total number of columns.
pub const NUM_PROGRAM_COLS: usize = ProgramColumnsView::<()>::NUMBER_OF_COLUMNS;

use crate::columns_view::{columns_view_impl, make_col_map};
use crate::linear_combination::Column;
use crate::linear_combination_typed::ColumnWithTypedInput;
use crate::program::columns::ProgramRom;
use crate::stark::mozak_stark::{ProgramMultTable, TableWithTypedOutput};

columns_view_impl!(ProgramMult);
make_col_map!(ProgramMult);
/// A Row of ROM generated from read-only memory
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct ProgramMult<T> {
    pub rom_row: ProgramRom<T>,
    pub mult_in_cpu: T,
}

#[must_use]
pub fn lookup_for_cpu() -> TableWithTypedOutput<ProgramRom<Column>> {
    ProgramMultTable::new(COL_MAP.rom_row, COL_MAP.mult_in_cpu)
}

#[must_use]
pub fn lookup_for_rom() -> TableWithTypedOutput<ProgramRom<Column>> {
    ProgramMultTable::new(COL_MAP.rom_row, ColumnWithTypedInput::constant(1))
}

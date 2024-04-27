use crate::columns_view::{columns_view_impl, make_col_map};
use crate::linear_combination::Column;
use crate::linear_combination_typed::ColumnWithTypedInput;
use crate::program::columns::ProgramRom;
use crate::stark::mozak_stark::{ProgramMultTable, TableWithTypedOutput};

columns_view_impl!(ProgramMult);
make_col_map!(ProgramMult);
/// A Row of ROM generated from read-only memory
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct ProgramMult<T> {
    pub inst: ProgramRom<T>,
    pub mult_in_cpu: T,
}

#[must_use]
pub fn lookup_for_cpu() -> TableWithTypedOutput<ProgramRom<Column>> {
    ProgramMultTable::new(COL_MAP.inst, COL_MAP.mult_in_cpu)
}

#[must_use]
pub fn lookup_for_rom() -> TableWithTypedOutput<ProgramRom<Column>> {
    ProgramMultTable::new(COL_MAP.inst, ColumnWithTypedInput::constant(1))
}

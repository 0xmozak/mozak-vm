use crate::columns_view::{columns_view_impl, make_col_map};
use crate::linear_combination::Column;
use crate::program::columns::InstructionRow;
use crate::stark::mozak_stark::{ProgramMultTable, TableWithTypedOutput};

columns_view_impl!(ProgramMult);
make_col_map!(ProgramMult);
/// A Row of ROM generated from read-only memory
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct ProgramMult<T> {
    pub inst: InstructionRow<T>,
    // TODO: see if we can get rid of this.
    // We could just force our programs to have a power of two length.
    pub mult_in_rom: T,
    pub mult_in_cpu: T,
}

#[must_use]
pub fn lookup_for_cpu() -> TableWithTypedOutput<InstructionRow<Column>> {
    ProgramMultTable::new(COL_MAP.inst, COL_MAP.mult_in_cpu)
}

#[must_use]
pub fn lookup_for_rom() -> TableWithTypedOutput<InstructionRow<Column>> {
    ProgramMultTable::new(COL_MAP.inst, COL_MAP.mult_in_rom)
}

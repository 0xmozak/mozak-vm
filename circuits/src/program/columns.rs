use itertools::izip;
use plonky2::hash::hash_types::RichField;

use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};
use crate::cpu::columns::Instruction;
use crate::generation::ascending_sum;
use crate::linear_combination::Column;
use crate::linear_combination_typed::ColumnWithTypedInput;
use crate::stark::mozak_stark::{ProgramTable, TableWithTypedOutput};

columns_view_impl!(ProgramRom);
make_col_map!(ProgramRom);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
/// A Row of ROM generated from read-only memory
pub struct ProgramRom<T> {
    // Design doc for CPU <> Program cross-table-lookup:
    // https://www.notion.so/0xmozak/Cross-Table-Lookup-bbe98d9471114c36a278f0c491f203e5#c3876d13c1f94b7ab154ea1f8b908181
    pub pc: T,
    /// `inst_data` include:
    /// - ops: This is an internal opcode, not the opcode from RISC-V
    /// - `is_op1_signed` and `is_op2_signed`
    /// - `rs1_select`, `rs2_select`, and `rd_select`
    /// - `imm_value`
    pub inst_data: T,
}

impl<F: RichField> From<Instruction<F>> for ProgramRom<F> {
    fn from(inst: Instruction<F>) -> Self {
        pub fn reduce_with_powers<F: RichField, I: IntoIterator<Item = F>>(
            terms: I,
            alpha: u64,
        ) -> F {
            izip!((0..).map(|i| F::from_canonical_u64(alpha.pow(i))), terms)
                .map(|(base, val)| base * val)
                .sum()
        }

        Self {
            pc: inst.pc,
            inst_data: reduce_with_powers(
                [
                    ascending_sum(inst.ops),
                    inst.is_op1_signed,
                    inst.is_op2_signed,
                    inst.rs1_selected,
                    inst.rs2_selected,
                    inst.rd_selected,
                    inst.imm_value,
                ],
                1 << 5,
            ),
        }
    }
}

// Total number of columns.
pub const NUM_PROGRAM_COLS: usize = ProgramRom::<()>::NUMBER_OF_COLUMNS;

#[must_use]
pub fn lookup_for_ctl() -> TableWithTypedOutput<ProgramRom<Column>> {
    ProgramTable::new(COL_MAP, ColumnWithTypedInput::constant(1))
}

pub mod stark;

pub mod columns {

    use crate::columns_view::{columns_view_impl, make_col_map};
    use crate::cpu_skeleton::columns::CpuSkeletonCtl;
    use crate::linear_combination::Column;
    use crate::linear_combination_typed::ColumnWithTypedInput;
    use crate::memory::columns::MemoryCtl;
    use crate::program::columns::ProgramRom;
    use crate::rangecheck::columns::RangeCheckCtl;
    // use crate::rangecheck::columns::RangeCheckCtl;
    use crate::register::RegisterCtl;
    use crate::stark::mozak_stark::{StoreWordTable, TableWithTypedOutput};

    columns_view_impl!(Instruction);
    #[repr(C)]
    #[derive(Clone, Copy, Eq, PartialEq, Debug)]
    pub struct Instruction<T> {
        /// The original instruction (+ `imm_value`) used for program
        /// cross-table-lookup.
        pub pc: T,
        /// Selects the register to use as source for `rs1`
        pub rs1_selected: T,
        /// Selects the register to use as source for `rs2`
        pub rs2_selected: T,
        /// Special immediate value used for code constants
        pub imm_value: T,
    }

    make_col_map!(StoreWord);
    columns_view_impl!(StoreWord);
    #[repr(C)]
    #[derive(Clone, Copy, Eq, PartialEq, Debug)]
    pub struct StoreWord<T> {
        pub inst: Instruction<T>,
        pub clk: T,
        pub op1_limbs: [T; 4],
        pub op2_value: T,
        // Extra column, so we can do CTL, like range check and memory.
        pub address: T,

        pub is_running: T,
    }

    #[must_use]
    pub fn register_looking() -> Vec<TableWithTypedOutput<RegisterCtl<Column>>> {
        let is_read = ColumnWithTypedInput::constant(1);

        vec![
            StoreWordTable::new(
                RegisterCtl {
                    clk: COL_MAP.clk,
                    op: is_read,
                    addr: COL_MAP.inst.rs1_selected,
                    value: ColumnWithTypedInput::reduce_with_powers(COL_MAP.op1_limbs, 1 << 8),
                },
                COL_MAP.is_running,
            ),
            StoreWordTable::new(
                RegisterCtl {
                    clk: COL_MAP.clk,
                    op: is_read,
                    addr: COL_MAP.inst.rs2_selected,
                    value: COL_MAP.op2_value,
                },
                COL_MAP.is_running,
            ),
        ]
    }

    #[must_use]
    pub fn rangecheck_looking() -> Vec<TableWithTypedOutput<RangeCheckCtl<Column>>> {
        vec![StoreWordTable::new(
            RangeCheckCtl(COL_MAP.address),
            COL_MAP.is_running,
        )]
    }

    #[must_use]
    pub fn lookup_for_skeleton() -> TableWithTypedOutput<CpuSkeletonCtl<Column>> {
        StoreWordTable::new(
            CpuSkeletonCtl {
                clk: COL_MAP.clk,
                pc: COL_MAP.inst.pc,
                new_pc: COL_MAP.inst.pc + 4,
                will_halt: ColumnWithTypedInput::constant(0),
            },
            COL_MAP.is_running,
        )
    }

    #[must_use]
    pub fn lookup_for_program_rom() -> TableWithTypedOutput<ProgramRom<Column>> {
        let inst = COL_MAP.inst;
        StoreWordTable::new(
            ProgramRom {
                pc: inst.pc,
                // Combine columns into a single column.
                // - ops: This is an internal opcode, not the opcode from RISC-V, and can fit within
                //   5 bits.
                // - is_op1_signed and is_op2_signed: These fields occupy 1 bit each.
                // - rs1_select, rs2_select, and rd_select: These fields require 5 bits each.
                // - imm_value: This field requires 32 bits.
                // Therefore, the total bit requirement is 5 * 6 + 32 = 62 bits, which is less than
                // the size of the Goldilocks field.
                // Note: The imm_value field, having more than 5 bits, must be positioned as the
                // last column in the list to ensure the correct functioning of
                // 'reduce_with_powers'.
                inst_data: ColumnWithTypedInput::reduce_with_powers(
                    [
                        // TODO: don't hard-code opcode like this.
                        ColumnWithTypedInput::constant(18),
                        // TODO: use a struct here to name the components, and make IntoIterator,
                        // like we do with our stark tables.
                        ColumnWithTypedInput::constant(0),
                        ColumnWithTypedInput::constant(0),
                        inst.rs1_selected,
                        inst.rs2_selected,
                        ColumnWithTypedInput::constant(0),
                        inst.imm_value,
                    ],
                    1 << 5,
                ),
            },
            COL_MAP.is_running,
        )
    }

    /// Lookup between Store Word memory table
    /// and Memory stark table.
    #[must_use]
    pub fn lookup_for_memory_limb() -> Vec<TableWithTypedOutput<MemoryCtl<Column>>> {
        (0..4)
            .map(|limb_index| {
                StoreWordTable::new(
                    MemoryCtl {
                        clk: COL_MAP.clk,
                        is_store: ColumnWithTypedInput::constant(1),
                        is_load: ColumnWithTypedInput::constant(0),
                        value: COL_MAP.op1_limbs[limb_index],
                        addr: COL_MAP.address + i64::try_from(limb_index).unwrap(),
                    },
                    COL_MAP.is_running,
                )
            })
            .collect()
    }
}

use columns::{Instruction, StoreWord};
use mozak_runner::instruction::Op;
use mozak_runner::vm::Row;
use plonky2::hash::hash_types::RichField;

use crate::utils::pad_trace_with_default;

#[must_use]
pub fn generate<F: RichField>(executed: &[Row<F>]) -> Vec<StoreWord<F>> {
    pad_trace_with_default(
        executed
            .iter()
            .filter(|row| (Op::SW == row.instruction.op))
            .map(
                |Row {
                     state,
                     instruction: inst,
                     aux,
                 }| {
                    let rs1_selected = inst.args.rs1;
                    let rs2_selected = inst.args.rs2;
                    let op1_value = state.get_register_value(rs1_selected);
                    let op2_value = state.get_register_value(rs2_selected);
                    let imm_value = inst.args.imm;
                    let address = aux.mem.unwrap().addr;
                    assert_eq!(address, op2_value.wrapping_add(imm_value));
                    let op1_limbs = op1_value.to_le_bytes().map(u32::from);

                    StoreWord {
                        inst: Instruction {
                            pc: state.get_pc(),
                            rs1_selected: u32::from(rs1_selected),
                            rs2_selected: u32::from(rs2_selected),
                            imm_value,
                        },
                        clk: u32::try_from(state.clk).unwrap(),
                        op1_limbs,
                        op2_value,
                        address,
                        is_running: 1,
                    }
                    .map(F::from_canonical_u32)
                },
            )
            .collect(),
    )
}

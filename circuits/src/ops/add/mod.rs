pub mod stark;

pub mod columns {

    use crate::columns_view::{columns_view_impl, make_col_map};
    use crate::cpu_skeleton::columns::CpuSkeletonCtl;
    use crate::linear_combination::Column;
    use crate::linear_combination_typed::ColumnWithTypedInput;
    use crate::program::columns::ProgramRom;
    use crate::rangecheck::columns::RangeCheckCtl;
    use crate::register::RegisterCtl;
    use crate::stark::mozak_stark::{AddTable, TableWithTypedOutput};

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
        /// Selects the register to use as destination for `rd`
        pub rd_selected: T,
        /// Special immediate value used for code constants
        pub imm_value: T,
    }

    make_col_map!(Add);
    columns_view_impl!(Add);
    #[repr(C)]
    #[derive(Clone, Copy, Eq, PartialEq, Debug)]
    pub struct Add<T> {
        pub inst: Instruction<T>,
        // TODO(Matthias): could we get rid of the clk here?
        pub clk: T,
        pub op1_value: T,
        pub op2_value: T,
        pub dst_value: T,

        pub is_running: T,
    }

    const ADD: Add<ColumnWithTypedInput<Add<i64>>> = COL_MAP;

    #[must_use]
    pub fn register_looking() -> Vec<TableWithTypedOutput<RegisterCtl<Column>>> {
        let is_read = ColumnWithTypedInput::constant(1);
        let is_write = ColumnWithTypedInput::constant(2);

        vec![
            AddTable::new(
                RegisterCtl {
                    clk: ADD.clk,
                    op: is_read,
                    addr: ADD.inst.rs1_selected,
                    value: ADD.op1_value,
                },
                ADD.is_running,
            ),
            AddTable::new(
                RegisterCtl {
                    clk: ADD.clk,
                    op: is_read,
                    addr: ADD.inst.rs2_selected,
                    value: ADD.op2_value,
                },
                ADD.is_running,
            ),
            AddTable::new(
                RegisterCtl {
                    clk: ADD.clk,
                    op: is_write,
                    addr: ADD.inst.rd_selected,
                    value: ADD.dst_value,
                },
                ADD.is_running,
            ),
        ]
    }

    // We explicitly range check our output here, so we have the option of not doing
    // it for other operations that don't need it.
    #[must_use]
    pub fn rangecheck_looking() -> Vec<TableWithTypedOutput<RangeCheckCtl<Column>>> {
        vec![AddTable::new(RangeCheckCtl(ADD.dst_value), ADD.is_running)]
    }

    #[must_use]
    pub fn lookup_for_skeleton() -> TableWithTypedOutput<CpuSkeletonCtl<Column>> {
        AddTable::new(
            CpuSkeletonCtl {
                clk: ADD.clk,
                pc: ADD.inst.pc,
                new_pc: ADD.inst.pc + 4,
                will_halt: ColumnWithTypedInput::constant(0),
            },
            ADD.is_running,
        )
    }

    #[must_use]
    pub fn lookup_for_program_rom() -> TableWithTypedOutput<ProgramRom<Column>> {
        let inst = ADD.inst;
        AddTable::new(
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
                        // TODO: don't hard-code ADD like this.
                        ColumnWithTypedInput::constant(0),
                        // TODO: use a struct here to name the components, and make IntoIterator,
                        // like we do with our stark tables.
                        ColumnWithTypedInput::constant(0),
                        ColumnWithTypedInput::constant(0),
                        inst.rs1_selected,
                        inst.rs2_selected,
                        inst.rd_selected,
                        inst.imm_value,
                    ],
                    1 << 5,
                ),
            },
            ADD.is_running,
        )
    }
}

use columns::{Add, Instruction};
use mozak_runner::instruction::Op;
use mozak_runner::vm::{ExecutionRecord, Row};
use plonky2::hash::hash_types::RichField;

use crate::utils::pad_trace_with_default;

#[must_use]
pub fn generate<F: RichField>(record: &ExecutionRecord<F>) -> Vec<Add<F>> {
    let mut trace: Vec<Add<F>> = vec![];
    for Row {
        state,
        instruction: inst,
        aux,
    } in &record.executed
    {
        if let Op::ADD = inst.op {
            let row = Add {
                inst: Instruction {
                    pc: state.get_pc(),
                    rs1_selected: u32::from(inst.args.rs1),
                    rs2_selected: u32::from(inst.args.rs2),
                    rd_selected: u32::from(inst.args.rd),
                    imm_value: inst.args.imm,
                },
                // TODO: fix this, or change clk to u32?
                clk: u32::try_from(state.clk).unwrap(),
                op1_value: state.get_register_value(inst.args.rs1),
                op2_value: state.get_register_value(inst.args.rs2),
                dst_value: aux.dst_val,
                is_running: 1,
            }
            .map(F::from_canonical_u32);
            trace.push(row);
        }
    }
    pad_trace_with_default(trace)
}

pub mod stark;

pub mod columns {

    use crate::columns_view::{columns_view_impl, make_col_map};
    use crate::cpu_skeleton::columns::CpuSkeletonCtl;
    use crate::linear_combination::Column;
    use crate::linear_combination_typed::ColumnWithTypedInput;
    use crate::rangecheck::columns::RangeCheckCtl;
    use crate::register::RegisterCtl;
    use crate::stark::mozak_stark::{BltTakenTable, TableWithTypedOutput};

    columns_view_impl!(Instruction);
    #[repr(C)]
    #[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
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

    make_col_map!(BltTaken);
    columns_view_impl!(BltTaken);
    #[repr(C)]
    #[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
    pub struct BltTaken<T> {
        pub inst: Instruction<T>,
        // TODO(Matthias): could we get rid of the clk here?
        pub clk: T,
        pub op1_value: T,
        pub op2_value: T,

        pub is_running: T,
    }

    #[must_use]
    pub fn register_looking() -> Vec<TableWithTypedOutput<RegisterCtl<Column>>> {
        let is_read = ColumnWithTypedInput::constant(1);

        vec![
            BltTakenTable::new(
                RegisterCtl {
                    clk: COL_MAP.clk,
                    op: is_read,
                    addr: COL_MAP.inst.rs1_selected,
                    value: COL_MAP.op1_value,
                },
                COL_MAP.is_running,
            ),
            BltTakenTable::new(
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

    // We explicitly range check our output here, so we have the option of not doing
    // it for other operations that don't need it.
    #[must_use]
    pub fn rangecheck_looking() -> Vec<TableWithTypedOutput<RangeCheckCtl<Column>>> {
        // TODO: add an impl to be able to subtract (and add etc) i64 from columns.
        vec![BltTakenTable::new(
            RangeCheckCtl(
                COL_MAP.op2_value - COL_MAP.op1_value - ColumnWithTypedInput::constant(1),
            ),
            COL_MAP.is_running,
        )]
    }

    #[must_use]
    pub fn lookup_for_skeleton() -> TableWithTypedOutput<CpuSkeletonCtl<Column>> {
        BltTakenTable::new(
            CpuSkeletonCtl {
                clk: COL_MAP.clk,
                pc: COL_MAP.inst.pc,
                new_pc: COL_MAP.inst.imm_value,
                will_halt: ColumnWithTypedInput::constant(0),
            },
            COL_MAP.is_running,
        )
    }
}

use columns::{BltTaken, Instruction};
use mozak_runner::instruction::Op;
use mozak_runner::vm::{ExecutionRecord, Row};
use plonky2::hash::hash_types::RichField;

use crate::utils::pad_trace_with_default;

#[must_use]
pub fn generate<F: RichField>(record: &ExecutionRecord<F>) -> Vec<BltTaken<F>> {
    let mut trace: Vec<BltTaken<F>> = vec![];
    for Row {
        state,
        instruction: inst,
        ..
    } in &record.executed
    {
        let op1_value = state.get_register_value(inst.args.rs1);
        let op2_value = state.get_register_value(inst.args.rs2);
        // TOOD: add a helper in Aux whether the branch was taken, so we don't recreate
        // the logic here.
        if op1_value < op2_value && Op::BLTU == inst.op {
            let row = BltTaken {
                inst: Instruction {
                    pc: state.get_pc(),
                    rs1_selected: u32::from(inst.args.rs1),
                    rs2_selected: u32::from(inst.args.rs2),
                    imm_value: inst.args.imm,
                },
                // TODO: fix this, or change clk to u32?
                clk: u32::try_from(state.clk).unwrap(),
                op1_value: state.get_register_value(inst.args.rs1),
                op2_value: state.get_register_value(inst.args.rs2),
                is_running: 1,
            }
            .map(F::from_canonical_u32);
            trace.push(row);
        }
    }
    pad_trace_with_default(trace)
}

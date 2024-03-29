use std::marker::PhantomData;

use mozak_circuits_derive::StarkNameDisplay;
use mozak_runner::instruction::Op;
use mozak_runner::vm::{ExecutionRecord, Row};
use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};
use starky::evaluation_frame::{StarkEvaluationFrame, StarkFrame};
use starky::stark::Stark;

use crate::columns_view::{columns_view_impl, make_col_map, HasNamedColumns, NumberOfColumns};
use crate::cpu_skeleton::columns::CpuSkeletonCtl;
use crate::linear_combination::Column;
use crate::linear_combination_typed::ColumnWithTypedInput;
use crate::rangecheck::columns::RangeCheckCtl;
use crate::register::columns::RegisterCtl;
use crate::stark::mozak_stark::{AddTable, TableWithTypedOutput};

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
    /// Selects the register to use as destination for `rd`
    pub rd_selected: T,
    /// Special immediate value used for code constants
    pub imm_value: T,
}

make_col_map!(Add);
columns_view_impl!(Add);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct Add<T> {
    pub inst: Instruction<T>,
    // TODO(Matthias): could we get rid of the clk here?
    pub clk: T,
    pub op1_value: T,
    pub op2_value: T,
    pub dst_value: T,

    pub is_running: T,
}

#[derive(Copy, Clone, Default, StarkNameDisplay)]
#[allow(clippy::module_name_repetitions)]
pub struct AddStark<F, const D: usize> {
    pub _f: PhantomData<F>,
}

impl<F, const D: usize> HasNamedColumns for AddStark<F, D> {
    type Columns = Add<F>;
}

const COLUMNS: usize = Add::<()>::NUMBER_OF_COLUMNS;
const PUBLIC_INPUTS: usize = 0;

impl<F: RichField + Extendable<D>, const D: usize> Stark<F, D> for AddStark<F, D> {
    type EvaluationFrame<FE, P, const D2: usize> = StarkFrame<P, P::Scalar, COLUMNS, PUBLIC_INPUTS>

  where
      FE: FieldExtension<D2, BaseField = F>,
      P: PackedField<Scalar = FE>;
    type EvaluationFrameTarget =
        StarkFrame<ExtensionTarget<D>, ExtensionTarget<D>, COLUMNS, PUBLIC_INPUTS>;

    fn eval_packed_generic<FE, P, const D2: usize>(
        &self,
        vars: &Self::EvaluationFrame<FE, P, D2>,
        yield_constr: &mut ConstraintConsumer<P>,
    ) where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>, {
        let lv: &Add<P> = vars.get_local_values().into();
        let wrap_at = P::Scalar::from_noncanonical_u64(1 << 32);
        let added = lv.op1_value + lv.op2_value;
        let wrapped = added - wrap_at;

        // Check: the resulting sum is wrapped if necessary.
        // As the result is range checked, this make the choice deterministic,
        // even for a malicious prover.
        yield_constr.constraint((lv.dst_value - added) * (lv.dst_value - wrapped));
    }

    fn eval_ext_circuit(
        &self,
        _builder: &mut CircuitBuilder<F, D>,
        _vars: &Self::EvaluationFrameTarget,
        _yield_constr: &mut RecursiveConstraintConsumer<F, D>,
    ) {
        todo!()
    }

    fn constraint_degree(&self) -> usize { 3 }
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
pub fn generate<F: RichField>(record: &ExecutionRecord<F>) -> Vec<Add<F>> {
    let mut trace: Vec<Add<F>> = vec![];
    let ExecutionRecord { executed, .. } = record;
    for Row {
        state,
        instruction: inst,
        aux,
    } in executed
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
    trace
}

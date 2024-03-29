use std::marker::PhantomData;

use mozak_circuits_derive::StarkNameDisplay;
use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};
use starky::evaluation_frame::{StarkEvaluationFrame, StarkFrame};
use starky::stark::Stark;

use crate::columns_view::{columns_view_impl, HasNamedColumns, NumberOfColumns};

columns_view_impl!(Instruction);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct Instruction<T> {
    /// The original instruction (+ `imm_value`) used for program
    /// cross-table-lookup.
    pub pc: T,

    /// Selects the current operation type
    // pub ops: OpSelectors<T>,
    pub is_op1_signed: T,
    pub is_op2_signed: T,
    pub is_dst_signed: T,
    /// Selects the register to use as source for `rs1`
    pub rs1_selected: T,
    /// Selects the register to use as source for `rs2`
    pub rs2_selected: T,
    /// Selects the register to use as destination for `rd`
    pub rd_selected: T,
    /// Special immediate value used for code constants
    pub imm_value: T,
}

columns_view_impl!(Add);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct Add<T> {
    pub inst: Instruction<T>,
    pub op1_value: T,
    pub op2_value: T,
    pub dst_value: T,
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

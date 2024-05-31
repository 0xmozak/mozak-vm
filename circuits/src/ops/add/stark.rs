use std::marker::PhantomData;

use expr::{Expr, ExprBuilder, StarkFrameTyped};
use mozak_circuits_derive::StarkNameDisplay;
use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};
use starky::evaluation_frame::StarkFrame;
use starky::stark::Stark;

use super::columns::Add;
use crate::columns_view::{HasNamedColumns, NumberOfColumns};
use crate::expr::{build_ext, build_packed, ConstraintBuilder, GenerateConstraints};
use crate::unstark::NoColumns;

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

impl<'a, F, T: Copy, U, const D: usize>
    GenerateConstraints<'a, T, Add<Expr<'a, T>>, NoColumns<U>> for AddStark<F, { D }>
{
    fn generate_constraints(
        vars: &StarkFrameTyped<Add<Expr<'a, T>>, NoColumns<U>>,
    ) -> ConstraintBuilder<Expr<'a, T>> {
        let lv = vars.local_values;
        let mut constraints = ConstraintBuilder::default();

        let added = lv.op1_value + lv.op2_value + lv.inst.imm_value;
        let wrapped = added - (1 << 32);

        // Check: the resulting sum is wrapped if necessary.
        // As the result is range checked, this make the choice deterministic,
        // even for a malicious prover.
        constraints.always((lv.dst_value - added) * (lv.dst_value - wrapped));

        constraints
    }
}

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
        constraint_consumer: &mut ConstraintConsumer<P>,
    ) where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>, {
        let expr_builder = ExprBuilder::default();
        let constraints = Self::generate_constraints(&expr_builder.to_typed_starkframe(vars));
        build_packed(constraints, constraint_consumer);
    }

    fn eval_ext_circuit(
        &self,
        circuit_builder: &mut CircuitBuilder<F, D>,
        vars: &Self::EvaluationFrameTarget,
        constraint_consumer: &mut RecursiveConstraintConsumer<F, D>,
    ) {
        let expr_builder = ExprBuilder::default();
        let constraints = Self::generate_constraints(&expr_builder.to_typed_starkframe(vars));
        build_ext(constraints, circuit_builder, constraint_consumer);
    }

    fn constraint_degree(&self) -> usize { 3 }
}

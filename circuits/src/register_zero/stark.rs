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

use super::columns::RegisterZero;
use crate::columns_view::{HasNamedColumns, NumberOfColumns};
use crate::generation::instruction::ascending_sum;
use crate::register::columns::Ops;

#[derive(Clone, Copy, Default, StarkNameDisplay)]
#[allow(clippy::module_name_repetitions)]
pub struct RegisterZeroStark<F, const D: usize>(PhantomData<F>);

impl<F, const D: usize> HasNamedColumns for RegisterZeroStark<F, D> {
    type Columns = RegisterZero<F>;
}

const COLUMNS: usize = RegisterZero::<()>::NUMBER_OF_COLUMNS;
const PUBLIC_INPUTS: usize = 0;

impl<F: RichField + Extendable<D>, const D: usize> Stark<F, D> for RegisterZeroStark<F, D> {
    type EvaluationFrame<FE, P, const D2: usize> = StarkFrame<P, P::Scalar, COLUMNS, PUBLIC_INPUTS>

    where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>;
    type EvaluationFrameTarget =
        StarkFrame<ExtensionTarget<D>, ExtensionTarget<D>, COLUMNS, PUBLIC_INPUTS>;

    /// Constraints for the [`RegisterZeroStark`]
    fn eval_packed_generic<FE, P, const D2: usize>(
        &self,
        vars: &Self::EvaluationFrame<FE, P, D2>,
        yield_constr: &mut ConstraintConsumer<P>,
    ) where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>, {
        let lv: &RegisterZero<P> = vars.get_local_values().into();
        yield_constr.constraint(
            lv.value * (lv.op - P::Scalar::from_basefield(ascending_sum(Ops::write()))),
        );
    }

    fn eval_ext_circuit(
        &self,
        builder: &mut CircuitBuilder<F, D>,
        vars: &Self::EvaluationFrameTarget,
        yield_constr: &mut RecursiveConstraintConsumer<F, D>,
    ) {
        let lv: &RegisterZero<_> = vars.get_local_values().into();
        let write =
            builder.constant_extension(F::Extension::from_basefield(ascending_sum(Ops::write())));
        let op_is_write = builder.sub_extension(lv.op, write);
        let disjunction = builder.mul_extension(lv.value, op_is_write);
        yield_constr.constraint(builder, disjunction);
    }

    fn constraint_degree(&self) -> usize { 3 }
}

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

use super::columns::ProgramMult;
use crate::columns_view::{HasNamedColumns, NumberOfColumns};
use crate::expr::{build_ext, build_packed, ConstraintBuilder};
use crate::unstark::NoColumns;

#[derive(Clone, Copy, Default, StarkNameDisplay)]
#[allow(clippy::module_name_repetitions)]
pub struct ProgramMultStark<F, const D: usize> {
    pub _f: PhantomData<F>,
}

impl<F, const D: usize> HasNamedColumns for ProgramMultStark<F, D> {
    type Columns = ProgramMult<F>;
}

const COLUMNS: usize = ProgramMult::<()>::NUMBER_OF_COLUMNS;
const PUBLIC_INPUTS: usize = 0;

fn generate_constraints<'a, T: Copy>(
    vars: &StarkFrameTyped<ProgramMult<Expr<'a, T>>, NoColumns<Expr<'a, T>>>,
) -> ConstraintBuilder<Expr<'a, T>> {
    let lv = vars.local_values;
    let mut constraints = ConstraintBuilder::default();

    // Any instruction used in CPU should also be in the program
    constraints.always(lv.mult_in_cpu * (1 - lv.mult_in_rom));

    constraints
}

impl<F: RichField + Extendable<D>, const D: usize> Stark<F, D> for ProgramMultStark<F, D> {
    type EvaluationFrame<FE, P, const D2: usize> = StarkFrame<P, P::Scalar, COLUMNS, PUBLIC_INPUTS>

    where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>;
    type EvaluationFrameTarget =
        StarkFrame<ExtensionTarget<D>, ExtensionTarget<D>, COLUMNS, PUBLIC_INPUTS>;

    fn eval_packed_generic<FE, P, const D2: usize>(
        &self,
        vars: &Self::EvaluationFrame<FE, P, D2>,
        consumer: &mut ConstraintConsumer<P>,
    ) where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>, {
        let eb = ExprBuilder::default();
        let constraints = generate_constraints(&eb.to_typed_starkframe(vars));
        build_packed(constraints, consumer);
    }

    fn eval_ext_circuit(
        &self,
        builder: &mut CircuitBuilder<F, D>,
        vars: &Self::EvaluationFrameTarget,
        consumer: &mut RecursiveConstraintConsumer<F, D>,
    ) {
        let eb = ExprBuilder::default();
        let constraints = generate_constraints(&eb.to_typed_starkframe(vars));
        build_ext(constraints, builder, consumer);
    }

    fn constraint_degree(&self) -> usize { 3 }
}

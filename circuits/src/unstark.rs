use std::marker::PhantomData;

use expr::{Expr, StarkFrameTyped};
use mozak_circuits_derive::StarkNameDisplay;
use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};
use starky::evaluation_frame::StarkFrame;
use starky::stark::Stark;

use crate::columns_view::{columns_view_impl, HasNamedColumns, NumberOfColumns};
use crate::expr::{ConstraintBuilder, GenerateConstraints};

impl<'a, F, T, U, const D: usize, Columns, const COLUMNS: usize>
    GenerateConstraints<'a, T, U, Columns, NoColumns<U>> for Unstark<F, { D }, Columns, {COLUMNS} >
{
    fn generate_constraints(
        _vars: &StarkFrameTyped<Columns, NoColumns<U>>,
    ) -> ConstraintBuilder<Expr<'a, T>> {
        ConstraintBuilder::default()
    }
}

/// Template for a STARK with zero internal constraints. Use this if the STARK
/// itself does not need any built-in constraints, but rely on cross table
/// lookups for provability.
#[derive(Copy, Clone, Default, StarkNameDisplay)]
#[allow(clippy::module_name_repetitions)]
pub struct Unstark<F, const D: usize, Columns, const COLUMNS: usize> {
    pub _f: PhantomData<F>,
    pub _d: PhantomData<Columns>,
}

impl<F, const D: usize, Columns, const COLUMNS: usize> HasNamedColumns
    for Unstark<F, D, Columns, COLUMNS>
{
    type Columns = Columns;
}

const PUBLIC_INPUTS: usize = 0;

impl<
        F: RichField + Extendable<D>,
        const D: usize,
        Columns: Sync + NumberOfColumns,
        const COLUMNS: usize,
    > Stark<F, D> for Unstark<F, D, Columns, COLUMNS>
{
    type EvaluationFrame<FE, P, const D2: usize> = StarkFrame<P, P::Scalar, COLUMNS, PUBLIC_INPUTS>

    where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>;
    type EvaluationFrameTarget =
        StarkFrame<ExtensionTarget<D>, ExtensionTarget<D>, COLUMNS, PUBLIC_INPUTS>;

    fn eval_packed_generic<FE, P, const D2: usize>(
        &self,
        _vars: &Self::EvaluationFrame<FE, P, D2>,
        _constraint_consumer: &mut ConstraintConsumer<P>,
    ) where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>, {
    }

    fn eval_ext_circuit(
        &self,
        _builder: &mut CircuitBuilder<F, D>,
        _vars: &Self::EvaluationFrameTarget,
        _constraint_consumer: &mut RecursiveConstraintConsumer<F, D>,
    ) {
    }

    fn constraint_degree(&self) -> usize { 3 }
}

#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct NoColumns<T> {
    _phantom: PhantomData<T>,
}
columns_view_impl!(NoColumns);

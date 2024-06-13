use core::fmt::Debug;
use std::fmt::Display;
use std::marker::PhantomData;

use expr::{Expr, StarkFrameTyped};
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

impl<'a, F, NAME, T: Debug + 'a, const D: usize, Columns, const COLUMNS: usize>
    GenerateConstraints<'a, T> for Unstark<F, NAME, { D }, Columns, { COLUMNS }>
{
    type PublicInputs<E: Debug + 'a> = NoColumns<E>;
    type View<E: Debug + 'a> = NoColumns<E>;

    fn generate_constraints(
        _vars: &StarkFrameTyped<NoColumns<Expr<'a, T>>, NoColumns<Expr<'a, T>>>,
    ) -> ConstraintBuilder<Expr<'a, T>> {
        ConstraintBuilder::default()
    }
}

/// Template for a STARK with zero internal constraints. Use this if the STARK
/// itself does not need any built-in constraints, but rely on cross table
/// lookups for provability.
#[derive(Copy, Clone, Default)]
#[allow(clippy::module_name_repetitions)]
pub struct Unstark<F, NAME, const D: usize, Columns, const COLUMNS: usize> {
    pub _f: PhantomData<F>,
    pub _name: PhantomData<NAME>,
    pub _d: PhantomData<Columns>,
}

impl<F, NAME: Default + Debug, const D: usize, Columns, const COLUMNS: usize> Display
    for Unstark<F, NAME, D, Columns, COLUMNS>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", NAME::default())
    }
}

impl<F, NAME, const D: usize, Columns, const COLUMNS: usize> HasNamedColumns
    for Unstark<F, NAME, D, Columns, COLUMNS>
{
    type Columns = Columns;
}

const PUBLIC_INPUTS: usize = 0;

impl<
        F: RichField + Extendable<D>,
        NAME: Sync,
        const D: usize,
        Columns: Sync + NumberOfColumns,
        const COLUMNS: usize,
    > Stark<F, D> for Unstark<F, NAME, D, Columns, COLUMNS>
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

// Simple marco to create a type holding the name for the Unstark
macro_rules! impl_name {
    ($alias:ident, $name:ident) => {
        mod name {
            #[derive(Default, Debug, Clone, Copy)]
            pub struct $name {}
        }

        use name::$name as $alias;
    }
}

pub(crate) use impl_name;

#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct NoColumns<T> {
    _phantom: PhantomData<T>,
}
columns_view_impl!(NoColumns);

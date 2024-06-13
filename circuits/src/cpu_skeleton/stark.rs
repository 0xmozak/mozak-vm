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

use super::columns::CpuSkeleton;
use crate::columns_view::{HasNamedColumns, NumberOfColumns};
use crate::expr::{build_ext, build_packed, ConstraintBuilder, GenerateConstraints};
use crate::stark::mozak_stark::PublicInputs;

#[derive(Clone, Copy, Default, StarkNameDisplay)]
#[allow(clippy::module_name_repetitions)]
pub struct CpuSkeletonStark<F, const D: usize> {
    pub _f: PhantomData<F>,
}

impl<F, const D: usize> HasNamedColumns for CpuSkeletonStark<F, D> {
    type Columns = CpuSkeleton<F>;
}

const COLUMNS: usize = CpuSkeleton::<()>::NUMBER_OF_COLUMNS;
// Public inputs: [PC of the first row]
const PUBLIC_INPUTS: usize = PublicInputs::<()>::NUMBER_OF_COLUMNS;

impl<'a, F, T: Copy + 'a, const D: usize> GenerateConstraints<'a, T>
    for CpuSkeletonStark<F, { D }>
{
    type PublicInputs<E: 'a> = PublicInputs<E>;
    type View<E: 'a> = CpuSkeleton<E>;

    fn generate_constraints(
        vars: &StarkFrameTyped<CpuSkeleton<Expr<'a, T>>, PublicInputs<Expr<'a, T>>>,
    ) -> ConstraintBuilder<Expr<'a, T>> {
        let lv = vars.local_values;
        let nv = vars.next_values;
        let public_inputs = vars.public_inputs;
        let mut constraints = ConstraintBuilder::default();

        constraints.first_row(lv.pc - public_inputs.entry_point);
        // Clock starts at 2. This is to differentiate
        // execution clocks (2 and above) from
        // clk values `0` and `1` which are reserved for
        // elf initialisation and zero initialisation respectively.
        constraints.first_row(lv.clk - 2);

        let clock_diff = nv.clk - lv.clk;
        constraints.transition(clock_diff.is_binary());

        // clock only counts up when we are still running.
        constraints.transition(clock_diff - lv.is_running);

        // We start in running state.
        constraints.first_row(lv.is_running - 1);

        // We may transition to a non-running state.
        constraints.transition(nv.is_running * (nv.is_running - lv.is_running));

        // We end in a non-running state.
        constraints.last_row(lv.is_running);

        // NOTE: in our old CPU table we had constraints that made sure nothing
        // changes anymore, once we are halted. We don't need those
        // anymore: the only thing that can change are memory or registers.  And
        // our CTLs make sure, that after we are halted, no more memory
        // or register changes are allowed.
        constraints
    }
}

impl<F: RichField + Extendable<D>, const D: usize> Stark<F, D> for CpuSkeletonStark<F, D> {
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
        let expr_builder = ExprBuilder::default();
        let vars = expr_builder.to_typed_starkframe(vars);
        let constraints = Self::generate_constraints(&vars);
        build_packed(constraints, consumer);
    }

    fn eval_ext_circuit(
        &self,
        builder: &mut CircuitBuilder<F, D>,
        vars: &Self::EvaluationFrameTarget,
        consumer: &mut RecursiveConstraintConsumer<F, D>,
    ) {
        let eb = ExprBuilder::default();
        let vars = eb.to_typed_starkframe(vars);
        let constraints = Self::generate_constraints(&vars);
        build_ext(constraints, builder, consumer);
    }

    fn constraint_degree(&self) -> usize { 3 }
}

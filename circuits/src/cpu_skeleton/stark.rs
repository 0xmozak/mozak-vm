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

use super::columns::CpuSkeleton;
use crate::columns_view::{HasNamedColumns, NumberOfColumns};
use crate::cpu::stark::is_binary_transition;

#[derive(Clone, Copy, Default, StarkNameDisplay)]
#[allow(clippy::module_name_repetitions)]
pub struct CpuSkeletonStark<F, const D: usize> {
    pub _f: PhantomData<F>,
}

impl<F, const D: usize> HasNamedColumns for CpuSkeletonStark<F, D> {
    type Columns = CpuSkeleton<F>;
}

const COLUMNS: usize = CpuSkeleton::<()>::NUMBER_OF_COLUMNS;
const PUBLIC_INPUTS: usize = 0;

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
        yield_constr: &mut ConstraintConsumer<P>,
    ) where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>, {
        let lv: &CpuSkeleton<_> = vars.get_local_values().into();
        let nv: &CpuSkeleton<_> = vars.get_next_values().into();
        let clock_diff = nv.clk - lv.clk;
        is_binary_transition(yield_constr, clock_diff);

        // clock only counts up when we are still running.
        yield_constr.constraint_transition(clock_diff - lv.is_running);

        // We start in running state.
        yield_constr.constraint_first_row(lv.is_running - P::ONES);

        // We may transition to a non-running state.
        yield_constr.constraint_transition(nv.is_running * (nv.is_running - lv.is_running));

        // We end in a non-running state.
        yield_constr.constraint_last_row(lv.is_running);
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

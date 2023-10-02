use std::borrow::Borrow;
use std::fmt::Display;
use std::marker::PhantomData;

use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::hash::hash_types::RichField;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};
use starky::stark::Stark;
use starky::vars::{StarkEvaluationTargets, StarkEvaluationVars};

use super::columns::RangeCheckLimb;
use crate::columns_view::NumberOfColumns;

#[derive(Copy, Clone, Default)]
#[allow(clippy::module_name_repetitions)]
pub struct RangeCheckLimbStark<F, const D: usize> {
    pub _f: PhantomData<F>,
}

impl<F, const D: usize> Display for RangeCheckLimbStark<F, D> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RangeCheckLimbStark")
    }
}

impl<F: RichField + Extendable<D>, const D: usize> Stark<F, D> for RangeCheckLimbStark<F, D> {
    const COLUMNS: usize = RangeCheckLimb::<()>::NUMBER_OF_COLUMNS;
    const PUBLIC_INPUTS: usize = 0;

    fn eval_packed_generic<FE, P, const D2: usize>(
        &self,
        vars: StarkEvaluationVars<FE, P, { Self::COLUMNS }, { Self::PUBLIC_INPUTS }>,
        yield_constr: &mut ConstraintConsumer<P>,
    ) where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>, {
        let lv: &RangeCheckLimb<P> = vars.local_values.borrow();
        let nv: &RangeCheckLimb<P> = vars.next_values.borrow();
        // Check: the `element`s form a sequence from 0 to 255, with possible
        // duplicates.
        yield_constr.constraint_first_row(lv.element);
        yield_constr
            .constraint_transition((nv.element - lv.element - FE::ONE) * (nv.element - lv.element));
        yield_constr.constraint_last_row(lv.element - FE::from_canonical_u8(u8::MAX));
    }

    #[no_coverage]
    fn eval_ext_circuit(
        &self,
        _builder: &mut CircuitBuilder<F, D>,
        _vars: StarkEvaluationTargets<D, { Self::COLUMNS }, { Self::PUBLIC_INPUTS }>,
        _yield_constr: &mut RecursiveConstraintConsumer<F, D>,
    ) {
        unimplemented!()
    }

    fn constraint_degree(&self) -> usize { 3 }
}

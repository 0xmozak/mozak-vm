use std::borrow::Borrow;
use std::marker::PhantomData;

use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::hash::hash_types::RichField;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};
use starky::stark::Stark;
use starky::vars::{StarkEvaluationTargets, StarkEvaluationVars};

use super::columns::RangeCheckU16;
use crate::columns_view::NumberOfColumns;

#[derive(Copy, Clone, Default)]
#[allow(clippy::module_name_repetitions)]
pub struct RangeCheckLimbStark<F, const D: usize> {
    pub _f: PhantomData<F>,
}

impl<F: RichField + Extendable<D>, const D: usize> Stark<F, D> for RangeCheckLimbStark<F, D> {
    const COLUMNS: usize = RangeCheckU16::<()>::NUMBER_OF_COLUMNS;
    const PUBLIC_INPUTS: usize = 0;

    /// Given the u32 value and the u16 limbs found in our variables to be
    /// evaluated, perform:
    ///   1. sumcheck between val (u32) and limbs (u16),
    ///   2. rangecheck for limbs.
    fn eval_packed_generic<FE, P, const D2: usize>(
        &self,
        vars: StarkEvaluationVars<FE, P, { Self::COLUMNS }, { Self::PUBLIC_INPUTS }>,
        yield_constr: &mut ConstraintConsumer<P>,
    ) where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>, {
        let lv: &RangeCheckU16<P> = vars.local_values.borrow();
        let nv: &RangeCheckU16<P> = vars.next_values.borrow();
        // Check: the `fixed_range_check_u16` forms a sequence from 0 to 2^16-1.
        //  this column will be used to connect to the permutation columns,
        //  `fixed_range_check_u16_permuted_hi` and `fixed_range_check_u16_permuted_lo`.
        yield_constr.constraint_first_row(lv.range_check_u16);
        yield_constr.constraint_transition(
            (nv.range_check_u16 - lv.range_check_u16 - FE::ONE)
                * (nv.range_check_u16 - lv.range_check_u16),
        );
        yield_constr.constraint_last_row(lv.range_check_u16 - FE::from_canonical_u8(u8::MAX));
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

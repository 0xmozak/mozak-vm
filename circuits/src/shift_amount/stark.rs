use std::borrow::Borrow;
use std::marker::PhantomData;

use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::hash::hash_types::RichField;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};
use starky::permutation::PermutationPair;
use starky::stark::Stark;
use starky::vars::{StarkEvaluationTargets, StarkEvaluationVars};

use super::columns::{ShiftAmountView, MAP, NUM_SHAMT_COLS};
use crate::lookup::eval_lookups;

#[derive(Copy, Clone, Default)]
#[allow(clippy::module_name_repetitions)]
pub struct ShiftAmountStark<F, const D: usize> {
    pub _f: PhantomData<F>,
}

impl<F: RichField + Extendable<D>, const D: usize> Stark<F, D> for ShiftAmountStark<F, D> {
    const COLUMNS: usize = NUM_SHAMT_COLS;
    const PUBLIC_INPUTS: usize = 0;

    fn eval_packed_generic<FE, P, const D2: usize>(
        &self,
        vars: StarkEvaluationVars<FE, P, { Self::COLUMNS }, { Self::PUBLIC_INPUTS }>,
        yield_constr: &mut ConstraintConsumer<P>,
    ) where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>, {
        let lv: &ShiftAmountView<P> = vars.local_values.borrow();
        let nv: &ShiftAmountView<P> = vars.next_values.borrow();

        // Constraints on shift amount
        yield_constr.constraint_first_row(lv.fixed_shamt);
        yield_constr.constraint_transition(
            (nv.fixed_shamt - lv.fixed_shamt - P::ONES) * (nv.fixed_shamt - lv.fixed_shamt),
        );
        yield_constr.constraint_last_row(lv.fixed_shamt - P::Scalar::from_canonical_u8(31));
        eval_lookups(
            vars,
            yield_constr,
            MAP.shamt_permuted,
            MAP.fixed_shamt_permuted,
        );

        // Constraints on multiplier
        let diff = nv.fixed_shamt - lv.fixed_shamt;
        yield_constr.constraint_first_row(lv.fixed_multiplier - P::ONES);
        yield_constr
            .constraint_transition(nv.fixed_multiplier - (P::ONES + diff) * lv.fixed_multiplier);
        yield_constr
            .constraint_last_row(lv.fixed_multiplier - P::Scalar::from_canonical_u32(1 << 31));
        eval_lookups(
            vars,
            yield_constr,
            MAP.multiplier_permuted,
            MAP.fixed_multiplier_permuted,
        );
    }

    fn permutation_pairs(&self) -> Vec<PermutationPair> {
        vec![
            PermutationPair::singletons(MAP.shamt, MAP.shamt_permuted),
            PermutationPair::singletons(MAP.multiplier, MAP.multiplier_permuted),
        ]
    }

    fn constraint_degree(&self) -> usize { 3 }

    #[no_coverage]
    fn eval_ext_circuit(
        &self,
        _builder: &mut CircuitBuilder<F, D>,
        _vars: StarkEvaluationTargets<D, { Self::COLUMNS }, { Self::PUBLIC_INPUTS }>,
        _yield_constr: &mut RecursiveConstraintConsumer<F, D>,
    ) {
        unimplemented!()
    }
}

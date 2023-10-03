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

use super::columns::MemoryInit;
use crate::columns_view::NumberOfColumns;
use crate::stark::utils::is_binary;

#[derive(Clone, Copy, Default)]
#[allow(clippy::module_name_repetitions)]
pub struct MemoryInitStark<F, const D: usize> {
    pub _f: PhantomData<F>,
}

impl<F, const D: usize> Display for MemoryInitStark<F, D> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MemoryInitStark")
    }
}

impl<F: RichField + Extendable<D>, const D: usize> Stark<F, D> for MemoryInitStark<F, D> {
    const COLUMNS: usize = MemoryInit::<F>::NUMBER_OF_COLUMNS;
    const PUBLIC_INPUTS: usize = 0;

    fn eval_packed_generic<FE, P, const D2: usize>(
        &self,
        vars: StarkEvaluationVars<FE, P, { Self::COLUMNS }, { Self::PUBLIC_INPUTS }>,
        yield_constr: &mut ConstraintConsumer<P>,
    ) where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>, {
        let lv: &MemoryInit<P> = vars.local_values.borrow();
        is_binary(yield_constr, lv.filter);
    }

    #[coverage(off)]
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

use plonky2::{
    field::{
        extension::{Extendable, FieldExtension},
        packed::PackedField,
    },
    hash::hash_types::RichField,
    plonk::config::GenericConfig,
};

use super::config::StarkConfig;
use super::constraint_consumer::ConstraintConsumer;
use super::stark::Stark;
use super::vars::StarkEvaluationVars;

pub(crate) fn eval_vanishing_poly<F, FE, P, C, S, const D: usize, const D2: usize>(
    stark: &S,
    _config: &StarkConfig,
    vars: StarkEvaluationVars<FE, P, { S::COLUMNS }>,
    consumer: &mut ConstraintConsumer<P>,
) where
    F: RichField + Extendable<D>,
    FE: FieldExtension<D2, BaseField = F>,
    P: PackedField<Scalar = FE>,
    C: GenericConfig<D, F = F>,
    S: Stark<F, D>,
{
    stark.eval_packed_generic(vars, consumer);
}

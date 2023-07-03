use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::hash::hash_types::RichField;
use starky::config::StarkConfig;
use starky::constraint_consumer::ConstraintConsumer;
use starky::stark::Stark;
use starky::vars::StarkEvaluationVars;

use super::permutation::{eval_permutation_checks, PermutationCheckVars};
use crate::cross_table_lookup::{eval_cross_table_lookup_checks, CtlCheckVars};

pub(crate) fn eval_vanishing_poly<F, FE, P, S, const D: usize, const D2: usize>(
    stark: &S,
    config: &StarkConfig,
    vars: StarkEvaluationVars<FE, P, { S::COLUMNS }, { S::PUBLIC_INPUTS }>,
    permutation_vars: Option<PermutationCheckVars<F, FE, P, D2>>,
    ctl_vars: &[CtlCheckVars<F, FE, P, D2>],
    consumer: &mut ConstraintConsumer<P>,
) where
    F: RichField + Extendable<D>,
    FE: FieldExtension<D2, BaseField = F>,
    P: PackedField<Scalar = FE>,
    S: Stark<F, D>,
{
    stark.eval_packed_generic(vars, consumer);
    if let Some(permutation_vars) = permutation_vars {
        eval_permutation_checks::<F, FE, P, S, D, D2>(
            stark,
            config,
            vars,
            permutation_vars,
            consumer,
        );
    }
    eval_cross_table_lookup_checks::<F, FE, P, S, D, D2>(vars, ctl_vars, consumer);
}

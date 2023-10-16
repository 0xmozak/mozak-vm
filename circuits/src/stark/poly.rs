#![allow(clippy::too_many_arguments)]

use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::field::polynomial::{PolynomialCoeffs, PolynomialValues};
use plonky2::field::zero_poly_coset::ZeroPolyOnCoset;
use plonky2::fri::oracle::PolynomialBatch;
use plonky2::hash::hash_types::RichField;
use plonky2::plonk::config::GenericConfig;
use plonky2::util::{log2_ceil, transpose};
use rayon::prelude::{IndexedParallelIterator, IntoParallelIterator, ParallelIterator};
use starky::config::StarkConfig;
use starky::constraint_consumer::ConstraintConsumer;
use starky::evaluation_frame::StarkEvaluationFrame;
use starky::stark::Stark;

use super::permutation::{eval_permutation_checks, PermutationCheckVars};
use crate::cross_table_lookup::{eval_cross_table_lookup_checks, CtlCheckVars, CtlData};
use crate::stark::permutation::challenge::GrandProductChallengeSet;

/// Computes the quotient polynomials `(sum alpha^i C_i(x)) / Z_H(x)` for
/// `alpha` in `alphas`, where the `C_i`s are the Stark constraints.
pub(crate) fn compute_quotient_polys<'a, F, P, C, S, const D: usize>(
    stark: &S,
    trace_commitment: &'a PolynomialBatch<F, C, D>,
    permutation_ctl_zs_commitment: &'a PolynomialBatch<F, C, D>,
    permutation_challenges: &'a [GrandProductChallengeSet<F>],
    public_inputs: &[F],
    ctl_data: &CtlData<F>,
    alphas: &[F],
    degree_bits: usize,
    num_permutation_zs: usize,
    config: &StarkConfig,
) -> Vec<PolynomialCoeffs<F>>
where
    F: RichField + Extendable<D>,
    P: PackedField<Scalar = F>,
    C: GenericConfig<D, F = F>,
    S: Stark<F, D>, {
    let degree = 1 << degree_bits;
    let rate_bits = config.fri_config.rate_bits;

    let quotient_degree_bits = log2_ceil(stark.quotient_degree_factor());
    assert!(
        quotient_degree_bits <= rate_bits,
        "Having constraints of degree higher than the rate is not supported yet."
    );
    let step = 1 << (rate_bits - quotient_degree_bits);
    // When opening the `Z`s polys at the "next" point, need to look at the point
    // `next_step` steps away.
    let next_step = 1 << quotient_degree_bits;

    // Evaluation of the first Lagrange polynomial on the LDE domain.
    let lagrange_first = PolynomialValues::selector(degree, 0).lde_onto_coset(quotient_degree_bits);
    // Evaluation of the last Lagrange polynomial on the LDE domain.
    let lagrange_last =
        PolynomialValues::selector(degree, degree - 1).lde_onto_coset(quotient_degree_bits);

    let z_h_on_coset = ZeroPolyOnCoset::<F>::new(degree_bits, quotient_degree_bits);

    // Retrieve the LDE values at index `i`.
    let get_trace_values_packed =
        |i_start| -> Vec<P> { trace_commitment.get_lde_values_packed(i_start, step) };

    // Last element of the subgroup.
    let last = F::primitive_root_of_unity(degree_bits).inverse();
    let size = degree << quotient_degree_bits;
    let coset = F::cyclic_subgroup_coset_known_order(
        F::primitive_root_of_unity(degree_bits + quotient_degree_bits),
        F::coset_shift(),
        size,
    );

    // We will step by `P::WIDTH`, and in each iteration, evaluate the quotient
    // polynomial at a batch of `P::WIDTH` points.
    let quotient_values = (0..size)
        .into_par_iter()
        .step_by(P::WIDTH)
        .flat_map_iter(|i_start| {
            let i_next_start = (i_start + next_step) % size;
            let i_range = i_start..i_start + P::WIDTH;

            println!("P::WIDTH: {:?}", P::WIDTH);
            println!("i_range: {:?}", i_range);
            println!("coset: {} {:?}", coset.len(), coset);
            // let x = *P::from_slice(&coset[i_range.clone()]);
            let x = *P::from_slice(&coset[i_range.clone()]);
            let z_last = x - last;
            let lagrange_basis_first = *P::from_slice(&lagrange_first.values[i_range.clone()]);
            let lagrange_basis_last = *P::from_slice(&lagrange_last.values[i_range]);

            let mut consumer = ConstraintConsumer::new(
                alphas.to_vec(),
                z_last,
                lagrange_basis_first,
                lagrange_basis_last,
            );
            let vars = StarkEvaluationFrame::from_values(
                &get_trace_values_packed(i_start),
                &get_trace_values_packed(i_next_start),
                public_inputs,
            );
            let permutation_check_vars = PermutationCheckVars {
                local_zs: permutation_ctl_zs_commitment.get_lde_values_packed(i_start, step)
                    [..num_permutation_zs]
                    .to_vec(),
                next_zs: permutation_ctl_zs_commitment.get_lde_values_packed(i_next_start, step)
                    [..num_permutation_zs]
                    .to_vec(),
                permutation_challenge_sets: permutation_challenges.to_owned(),
            };
            let ctl_vars = ctl_data
                .zs_columns
                .iter()
                .enumerate()
                .map(|(i, zs_columns)| CtlCheckVars::<F, F, P, 1> {
                    local_z: permutation_ctl_zs_commitment.get_lde_values_packed(i_start, step)
                        [num_permutation_zs + i],
                    next_z: permutation_ctl_zs_commitment.get_lde_values_packed(i_next_start, step)
                        [num_permutation_zs + i],
                    challenges: zs_columns.challenge,
                    columns: &zs_columns.columns,
                    filter_column: &zs_columns.filter_column,
                })
                .collect::<Vec<_>>();
            eval_vanishing_poly::<F, F, P, S, D, 1>(
                stark,
                config,
                &vars,
                permutation_check_vars,
                &ctl_vars,
                &mut consumer,
            );
            let mut constraints_evals = consumer.accumulators();
            // We divide the constraints evaluations by `Z_H(x)`.
            let denominator_inv: P = z_h_on_coset.eval_inverse_packed(i_start);
            for eval in &mut constraints_evals {
                *eval *= denominator_inv;
            }

            let num_challenges = alphas.len();

            (0..P::WIDTH).map(move |i| {
                (0..num_challenges)
                    .map(|j| constraints_evals[j].as_slice()[i])
                    .collect()
            })
        })
        .collect::<Vec<_>>();

    transpose(&quotient_values)
        .into_par_iter()
        .map(PolynomialValues::new)
        .map(|values| values.coset_ifft(F::coset_shift()))
        .collect()
}

pub(crate) fn eval_vanishing_poly<F, FE, P, S, const D: usize, const D2: usize>(
    stark: &S,
    config: &StarkConfig,
    vars: &S::EvaluationFrame<FE, P, D2>,
    permutation_vars: PermutationCheckVars<F, FE, P, D2>,
    ctl_vars: &[CtlCheckVars<F, FE, P, D2>],
    consumer: &mut ConstraintConsumer<P>,
) where
    F: RichField + Extendable<D>,
    FE: FieldExtension<D2, BaseField = F>,
    P: PackedField<Scalar = FE>,
    S: Stark<F, D>, {
    stark.eval_packed_generic(vars, consumer);
    eval_permutation_checks::<F, FE, P, S, D, D2>(stark, config, vars, permutation_vars, consumer);
    eval_cross_table_lookup_checks::<F, FE, P, S, D, D2>(vars, ctl_vars, consumer);
}

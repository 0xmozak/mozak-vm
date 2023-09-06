#![allow(clippy::too_many_arguments)]

use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::field::polynomial::{PolynomialCoeffs, PolynomialValues};
use plonky2::field::zero_poly_coset::ZeroPolyOnCoset;
use plonky2::fri::oracle::PolynomialBatch;
use plonky2::hash::hash_types::RichField;
use plonky2::plonk::config::GenericConfig;
use plonky2::util::{log2_ceil, transpose};
use plonky2_maybe_rayon::{IndexedParallelIterator, MaybeIntoParIter, ParallelIterator};
use starky::config::StarkConfig;
use starky::constraint_consumer::ConstraintConsumer;
use starky::stark::Stark;
use starky::vars::StarkEvaluationVars;

use crate::cross_table_lookup::{eval_cross_table_lookup_checks, CtlCheckVars, CtlData};
use crate::lookup::{Lookup, LookupCheckVars};
use crate::stark::prover::num_lookup_columns;

/// Computes the quotient polynomials `(sum alpha^i C_i(x)) / Z_H(x)` for
/// `alpha` in `alphas`, where the `C_i`s are the Stark constraints.
pub(crate) fn compute_quotient_polys<'a, F, P, C, S, const D: usize>(
    stark: &S,
    trace_commitment: &'a PolynomialBatch<F, C, D>,
    auxiliary_polys_commitment: &'a PolynomialBatch<F, C, D>,
    lookup_challenges: Option<&'a Vec<F>>,
    lookups: Option<&[Lookup]>,
    public_inputs: [F; S::PUBLIC_INPUTS],
    ctl_data: &CtlData<F>,
    alphas: &[F],
    degree_bits: usize,
    config: &StarkConfig,
) -> Vec<PolynomialCoeffs<F>>
where
    F: RichField + Extendable<D>,
    P: PackedField<Scalar = F>,
    C: GenericConfig<D, F = F>,
    S: Stark<F, D>,
    [(); S::COLUMNS]:,
    [(); S::PUBLIC_INPUTS]:, {
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
    let get_trace_values_packed = |i_start| -> [P; S::COLUMNS] {
        trace_commitment
            .get_lde_values_packed(i_start, step)
            .try_into()
            .unwrap()
    };

    // Last element of the subgroup.
    let last = F::primitive_root_of_unity(degree_bits).inverse();
    let size = degree << quotient_degree_bits;
    let coset = F::cyclic_subgroup_coset_known_order(
        F::primitive_root_of_unity(degree_bits + quotient_degree_bits),
        F::coset_shift(),
        size,
    );

    let num_lookup_columns = num_lookup_columns(lookups, config.num_challenges);

    println!("poly nluc; {}", num_lookup_columns);
    // We will step by `P::WIDTH`, and in each iteration, evaluate the quotient
    // polynomial at a batch of `P::WIDTH` points.
    let quotient_values = (0..size)
        .into_par_iter()
        .step_by(P::WIDTH)
        .flat_map_iter(|i_start| {
            let i_next_start = (i_start + next_step) % size;
            let i_range = i_start..i_start + P::WIDTH;

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
            let vars = StarkEvaluationVars {
                local_values: &get_trace_values_packed(i_start),
                next_values: &get_trace_values_packed(i_next_start),
                public_inputs: &public_inputs,
            };

            let lookup_vars = lookup_challenges.map(|challenges| LookupCheckVars {
                local_values: auxiliary_polys_commitment.get_lde_values_packed(i_start, step)
                    [..num_lookup_columns]
                    .to_vec(),
                next_values: auxiliary_polys_commitment.get_lde_values_packed(i_next_start, step)
                    [..num_lookup_columns]
                    .to_vec(),
                challenges: challenges.to_vec(),
            });

            let ctl_vars = ctl_data
                .zs_columns
                .iter()
                .enumerate()
                .map(|(i, zs_columns)| CtlCheckVars::<F, F, P, 1> {
                    local_z: auxiliary_polys_commitment.get_lde_values_packed(i_start, step)
                        [num_lookup_columns + i],
                    next_z: auxiliary_polys_commitment.get_lde_values_packed(i_next_start, step)
                        [num_lookup_columns + i],
                    challenges: zs_columns.challenge,
                    columns: &zs_columns.columns,
                    filter_column: &zs_columns.filter_column,
                })
                .collect::<Vec<_>>();
            eval_vanishing_poly::<F, F, P, S, D, 1>(
                stark,
                vars,
                lookups,
                lookup_vars,
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
    vars: StarkEvaluationVars<FE, P, { S::COLUMNS }, { S::PUBLIC_INPUTS }>,
    lookups: Option<&[Lookup]>,
    lookup_vars: Option<LookupCheckVars<F, FE, P, D2>>,
    ctl_vars: &[CtlCheckVars<F, FE, P, D2>],
    consumer: &mut ConstraintConsumer<P>,
) where
    F: RichField + Extendable<D>,
    FE: FieldExtension<D2, BaseField = F>,
    P: PackedField<Scalar = FE>,
    S: Stark<F, D>, {
    stark.eval_packed_generic(vars, consumer);

    // if let Some(lookups) = lookups {
    //     lookups
    //         .iter()
    //         .for_each(|l| l.eval(vars, &lookup_vars.as_ref().unwrap(), consumer))
    // }
    eval_cross_table_lookup_checks::<F, FE, P, S, D, D2>(vars, ctl_vars, consumer);
}

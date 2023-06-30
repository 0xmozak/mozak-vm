use std::fmt::Debug;

use itertools::Itertools;
use plonky2::field::batch_util::batch_multiply_inplace;
use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::field::polynomial::PolynomialCoeffs;
use plonky2::field::polynomial::PolynomialValues;
use plonky2::field::types::Field;
use plonky2::field::zero_poly_coset::ZeroPolyOnCoset;
use plonky2::fri::oracle::PolynomialBatch;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::config::GenericConfig;
use plonky2::util::reducing::{ReducingFactor, ReducingFactorTarget};
use plonky2::util::{log2_ceil, transpose};
use plonky2_maybe_rayon::*;
use starky::config::StarkConfig;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};
use starky::permutation::{PermutationCheckDataTarget, PermutationPair};
use starky::stark::Stark;
use starky::vars::{StarkEvaluationTargets, StarkEvaluationVars};

use super::prover::{GrandProductChallenge, GrandProductChallengeSet};
use crate::cross_table_lookup::{
    eval_cross_table_lookup_checks, eval_cross_table_lookup_checks_circuit, CtlCheckVars,
    CtlCheckVarsTarget, CtlData,
};

/// Computes the reduced polynomial, `\sum beta^i f_i(x) + gamma`, for both the
/// "left" and "right" sides of a given `PermutationPair`.
pub fn permutation_reduced_polys<F: Field>(
    instance: &PermutationInstance<F>,
    trace_poly_values: &[PolynomialValues<F>],
    degree: usize,
) -> (PolynomialValues<F>, PolynomialValues<F>) {
    let PermutationInstance {
        pair: PermutationPair { column_pairs },
        challenge: GrandProductChallenge { beta, gamma },
    } = instance;

    let mut reduced_lhs = PolynomialValues::constant(*gamma, degree);
    let mut reduced_rhs = PolynomialValues::constant(*gamma, degree);
    for ((lhs, rhs), weight) in column_pairs.iter().zip(beta.powers()) {
        reduced_lhs.add_assign_scaled(&trace_poly_values[*lhs], weight);
        reduced_rhs.add_assign_scaled(&trace_poly_values[*rhs], weight);
    }
    (reduced_lhs, reduced_rhs)
}

/// A single instance of a permutation check protocol.
pub(crate) struct PermutationInstance<'a, T: Copy + Eq + PartialEq + Debug> {
    pub(crate) pair: &'a PermutationPair,
    pub(crate) challenge: GrandProductChallenge<T>,
}

pub struct PermutationCheckVars<F, FE, P, const D2: usize>
where
    F: Field,
    FE: FieldExtension<D2, BaseField = F>,
    P: PackedField<Scalar = FE>,
{
    pub(crate) local_zs: Vec<P>,
    pub(crate) next_zs: Vec<P>,
    pub(crate) permutation_challenge_sets: Vec<GrandProductChallengeSet<F>>,
}

pub(crate) fn eval_permutation_checks<F, FE, P, S, const D: usize, const D2: usize>(
    stark: &S,
    config: &StarkConfig,
    vars: StarkEvaluationVars<FE, P, { S::COLUMNS }, { S::PUBLIC_INPUTS }>,
    permutation_vars: PermutationCheckVars<F, FE, P, D2>,
    consumer: &mut ConstraintConsumer<P>,
) where
    F: RichField + Extendable<D>,
    FE: FieldExtension<D2, BaseField = F>,
    P: PackedField<Scalar = FE>,
    S: Stark<F, D>,
{
    let PermutationCheckVars {
        local_zs,
        next_zs,
        permutation_challenge_sets,
    } = permutation_vars;

    // Check that Z(1) = 1;
    for &z in &local_zs {
        consumer.constraint_first_row(z - FE::ONE);
    }

    let permutation_pairs = stark.permutation_pairs();

    let permutation_batches = get_permutation_batches(
        &permutation_pairs,
        &permutation_challenge_sets,
        config.num_challenges,
        stark.permutation_batch_size(),
    );

    // Each zs value corresponds to a permutation batch.
    for (i, instances) in permutation_batches.iter().enumerate() {
        // Z(gx) * down = Z x  * up
        let (reduced_lhs, reduced_rhs): (Vec<P>, Vec<P>) = instances
            .iter()
            .map(|instance| {
                let PermutationInstance {
                    pair: PermutationPair { column_pairs },
                    challenge: GrandProductChallenge { beta, gamma },
                } = instance;
                let mut factor = ReducingFactor::new(*beta);
                let (lhs, rhs): (Vec<_>, Vec<_>) = column_pairs
                    .iter()
                    .map(|&(i, j)| (vars.local_values[i], vars.local_values[j]))
                    .unzip();
                (
                    factor.reduce_ext(lhs.into_iter()) + FE::from_basefield(*gamma),
                    factor.reduce_ext(rhs.into_iter()) + FE::from_basefield(*gamma),
                )
            })
            .unzip();
        let constraint = next_zs[i] * reduced_rhs.into_iter().product::<P>()
            - local_zs[i] * reduced_lhs.into_iter().product::<P>();
        consumer.constraint(constraint);
    }
}

pub(crate) fn eval_permutation_checks_circuit<F, S, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    stark: &S,
    config: &StarkConfig,
    vars: StarkEvaluationTargets<D, { S::COLUMNS }, { S::PUBLIC_INPUTS }>,
    permutation_data: PermutationCheckDataTarget<D>,
    consumer: &mut RecursiveConstraintConsumer<F, D>,
) where
    F: RichField + Extendable<D>,
    S: Stark<F, D>,
    [(); S::COLUMNS]:,
{
    let PermutationCheckDataTarget {
        local_zs,
        next_zs,
        permutation_challenge_sets,
    } = permutation_data;

    let one = builder.one_extension();
    // Check that Z(1) = 1;
    for &z in &local_zs {
        let z_1 = builder.sub_extension(z, one);
        consumer.constraint_first_row(builder, z_1);
    }

    let permutation_pairs = stark.permutation_pairs();

    let permutation_batches = get_permutation_batches(
        &permutation_pairs,
        &permutation_challenge_sets,
        config.num_challenges,
        stark.permutation_batch_size(),
    );

    // Each zs value corresponds to a permutation batch.
    for (i, instances) in permutation_batches.iter().enumerate() {
        let (reduced_lhs, reduced_rhs): (Vec<ExtensionTarget<D>>, Vec<ExtensionTarget<D>>) =
            instances
                .iter()
                .map(|instance| {
                    let PermutationInstance {
                        pair: PermutationPair { column_pairs },
                        challenge: GrandProductChallenge { beta, gamma },
                    } = instance;
                    let beta_ext = builder.convert_to_ext(*beta);
                    let gamma_ext = builder.convert_to_ext(*gamma);
                    let mut factor = ReducingFactorTarget::new(beta_ext);
                    let (lhs, rhs): (Vec<_>, Vec<_>) = column_pairs
                        .iter()
                        .map(|&(i, j)| (vars.local_values[i], vars.local_values[j]))
                        .unzip();
                    let reduced_lhs = factor.reduce(&lhs, builder);
                    let reduced_rhs = factor.reduce(&rhs, builder);
                    (
                        builder.add_extension(reduced_lhs, gamma_ext),
                        builder.add_extension(reduced_rhs, gamma_ext),
                    )
                })
                .unzip();
        let reduced_lhs_product = builder.mul_many_extension(reduced_lhs);
        let reduced_rhs_product = builder.mul_many_extension(reduced_rhs);
        // constraint = next_zs[i] * reduced_rhs_product - local_zs[i] *
        // reduced_lhs_product
        let constraint = {
            let tmp = builder.mul_extension(local_zs[i], reduced_lhs_product);
            builder.mul_sub_extension(next_zs[i], reduced_rhs_product, tmp)
        };
        consumer.constraint(builder, constraint)
    }
}

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

pub(crate) fn eval_vanishing_poly_circuit<F, S, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    stark: &S,
    config: &StarkConfig,
    vars: StarkEvaluationTargets<D, { S::COLUMNS }, { S::PUBLIC_INPUTS }>,
    permutation_data: Option<PermutationCheckDataTarget<D>>,
    ctl_vars: &[CtlCheckVarsTarget<F, D>],
    consumer: &mut RecursiveConstraintConsumer<F, D>,
) where
    F: RichField + Extendable<D>,
    S: Stark<F, D>,
    [(); S::COLUMNS]:,
    [(); S::PUBLIC_INPUTS]:,
{
    stark.eval_ext_circuit(builder, vars, consumer);
    if let Some(permutation_data) = permutation_data {
        eval_permutation_checks_circuit::<F, S, D>(
            builder,
            stark,
            config,
            vars,
            permutation_data,
            consumer,
        );
    }
    eval_cross_table_lookup_checks_circuit::<S, F, D>(builder, vars, ctl_vars, consumer);
}

/// Computes the quotient polynomials `(sum alpha^i C_i(x)) / Z_H(x)` for
/// `alpha` in `alphas`, where the `C_i`s are the Stark constraints.
pub(crate) fn compute_quotient_polys<'a, F, P, C, S, const D: usize>(
    stark: &S,
    trace_commitment: &'a PolynomialBatch<F, C, D>,
    permutation_ctl_zs_commitment: &'a PolynomialBatch<F, C, D>,
    permutation_challenges: Option<&'a Vec<GrandProductChallengeSet<F>>>,
    ctl_data: &CtlData<F>,
    public_inputs: [F; S::PUBLIC_INPUTS],
    alphas: Vec<F>,
    degree_bits: usize,
    num_permutation_zs: usize,
    config: &StarkConfig,
) -> Vec<PolynomialCoeffs<F>>
where
    F: RichField + Extendable<D>,
    P: PackedField<Scalar = F>,
    C: GenericConfig<D, F = F>,
    S: Stark<F, D>,
    [(); S::COLUMNS]:,
    [(); S::PUBLIC_INPUTS]:,
{
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
                alphas.clone(),
                z_last,
                lagrange_basis_first,
                lagrange_basis_last,
            );
            let vars = StarkEvaluationVars {
                local_values: &get_trace_values_packed(i_start),
                next_values: &get_trace_values_packed(i_next_start),
                public_inputs: &[],
            };
            let permutation_check_vars =
                permutation_challenges.map(|permutation_challenge_sets| PermutationCheckVars {
                    local_zs: permutation_ctl_zs_commitment.get_lde_values_packed(i_start, step)
                        [..num_permutation_zs]
                        .to_vec(),
                    next_zs: permutation_ctl_zs_commitment
                        .get_lde_values_packed(i_next_start, step)[..num_permutation_zs]
                        .to_vec(),
                    permutation_challenge_sets: permutation_challenge_sets.to_vec(),
                });
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
            eval_vanishing_poly::<F, FE, P, S, D, 0>(
                stark,
                config,
                vars,
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

/// Get a list of instances of our batch-permutation argument. These are
/// permutation arguments where the same `Z(x)` polynomial is used to check more
/// than one permutation. Before batching, each permutation pair leads to
/// `num_challenges` permutation arguments, so we start with the cartesian
/// product of `permutation_pairs` and `0..num_challenges`. Then we chunk these
/// arguments based on our batch size.
pub fn get_permutation_batches<'a, T: Copy + Debug + Eq>(
    permutation_pairs: &'a [PermutationPair],
    permutation_challenge_sets: &[GrandProductChallengeSet<T>],
    num_challenges: usize,
    batch_size: usize,
) -> Vec<Vec<PermutationInstance<'a, T>>> {
    permutation_pairs
        .iter()
        .cartesian_product(0..num_challenges)
        .chunks(batch_size)
        .into_iter()
        .map(|batch| {
            batch
                .enumerate()
                .map(|(i, (pair, chal))| {
                    let challenge = permutation_challenge_sets[i].challenges[chal];
                    PermutationInstance { pair, challenge }
                })
                .collect_vec()
        })
        .collect()
}

/// Compute all Z polynomials (for permutation arguments).
pub fn compute_permutation_z_polys<F, S, const D: usize>(
    stark: &S,
    config: &StarkConfig,
    trace_poly_values: &[PolynomialValues<F>],
    permutation_challenge_sets: &[GrandProductChallengeSet<F>],
) -> Vec<PolynomialValues<F>>
where
    F: RichField + Extendable<D>,
    S: Stark<F, D>,
{
    let permutation_pairs = stark.permutation_pairs();
    let permutation_batches = get_permutation_batches(
        &permutation_pairs,
        permutation_challenge_sets,
        config.num_challenges,
        stark.permutation_batch_size(),
    );

    permutation_batches
        .into_par_iter()
        .map(|instances| compute_permutation_z_poly(&instances, trace_poly_values))
        .collect()
}
/// Computes the elementwise product of a set of polynomials. Assumes that the
/// set is non-empty and that each polynomial has the same length.
fn poly_product_elementwise<F: Field>(
    mut polys: impl Iterator<Item = PolynomialValues<F>>,
) -> PolynomialValues<F> {
    let mut product = polys.next().expect("Expected at least one polynomial");
    for poly in polys {
        batch_multiply_inplace(&mut product.values, &poly.values)
    }
    product
}

/// Compute a single Z polynomial.
fn compute_permutation_z_poly<F: Field>(
    instances: &[PermutationInstance<F>],
    trace_poly_values: &[PolynomialValues<F>],
) -> PolynomialValues<F> {
    let degree = trace_poly_values[0].len();
    let (reduced_lhs_polys, reduced_rhs_polys): (Vec<_>, Vec<_>) = instances
        .iter()
        .map(|instance| permutation_reduced_polys(instance, trace_poly_values, degree))
        .unzip();

    let numerator = poly_product_elementwise(reduced_lhs_polys.into_iter());
    let denominator = poly_product_elementwise(reduced_rhs_polys.into_iter());

    // Compute the quotients.
    let denominator_inverses = F::batch_multiplicative_inverse(&denominator.values);
    let mut quotients = numerator.values;
    batch_multiply_inplace(&mut quotients, &denominator_inverses);

    // Compute Z, which contains partial products of the quotients.
    let mut partial_products = Vec::with_capacity(degree);
    let mut acc = F::ONE;
    for q in quotients {
        partial_products.push(acc);
        acc *= q;
    }
    PolynomialValues::new(partial_products)
}

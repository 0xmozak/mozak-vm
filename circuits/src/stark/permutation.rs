//! This module handles logic behind the permutation arguments:
//! challenge generation, batching, constraint generation and verification.

#![allow(clippy::module_name_repetitions)]

use std::fmt::Debug;

use itertools::Itertools;
use plonky2::field::batch_util::batch_multiply_inplace;
use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::field::polynomial::PolynomialValues;
use plonky2::field::types::Field;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::challenger::Challenger;
use plonky2::plonk::config::Hasher;
use plonky2::plonk::plonk_common::reduce_with_powers;
use plonky2::util::reducing::ReducingFactor;
use plonky2_maybe_rayon::{MaybeIntoParIter, ParallelIterator};
use starky::config::StarkConfig;
use starky::constraint_consumer::ConstraintConsumer;
use starky::permutation::PermutationPair;
use starky::stark::Stark;
use starky::vars::StarkEvaluationVars;

use crate::stark::permutation::challenge::{GrandProductChallenge, GrandProductChallengeSet};

pub(crate) mod challenge {
    use super::*;

    /// Randomness parameters that allow to generate based on a list of field
    /// values a unique value representation. If two lists of field values
    /// have the same representations, then, except for a small probability,
    /// the two lists are identical.
    ///
    /// To reduce the probability of failure, one can rerun the protocol
    /// multiple times, each time generating new [`beta`] and [`gamma`]. We
    /// do this in [`GrandProductChallengeSet`].
    ///
    /// In permutation check protocol instance we use this to make sure that
    /// values in all columns are the same.
    #[derive(Copy, Clone, Eq, PartialEq, Debug)]
    pub(crate) struct GrandProductChallenge<T: Copy + Eq + PartialEq + Debug> {
        /// Randomness used to combine multiple columns into one.
        pub(crate) beta: T,
        /// Random offset that's added to the beta-reduced column values.
        pub(crate) gamma: T,
    }

    impl<F: Field> GrandProductChallenge<F> {
        /// Calculate the unique value representation for the terms as:
        ///     `term_0*beta^n + term_1*beta^(n-1) + ... + term_n-1 + gamma`
        pub(crate) fn combine<'a, FE, P, T: IntoIterator<Item = &'a P>, const D2: usize>(
            &self,
            terms: T,
        ) -> P
        where
            FE: FieldExtension<D2, BaseField = F>,
            P: PackedField<Scalar = FE>,
            T::IntoIter: DoubleEndedIterator, {
            reduce_with_powers(terms, FE::from_basefield(self.beta))
                + FE::from_basefield(self.gamma)
        }
    }

    /// [`GrandProductChallenge`] repeated for [`num_challenges`] to boost
    /// soundness.
    #[derive(Clone, Eq, PartialEq, Debug)]
    pub(crate) struct GrandProductChallengeSet<T: Copy + Eq + PartialEq + Debug> {
        pub(crate) challenges: Vec<GrandProductChallenge<T>>,
    }

    pub(crate) trait GrandProductChallengeTrait<F: RichField, H: Hasher<F>> {
        fn get_grand_product_challenge(&mut self) -> GrandProductChallenge<F>;

        fn get_grand_product_challenge_set(
            &mut self,
            num_challenges: usize,
        ) -> GrandProductChallengeSet<F> {
            GrandProductChallengeSet {
                challenges: (0..num_challenges)
                    .map(|_| self.get_grand_product_challenge())
                    .collect(),
            }
        }

        fn get_n_grand_product_challenge_sets(
            &mut self,
            num_challenges: usize,
            num_sets: usize,
        ) -> Vec<GrandProductChallengeSet<F>> {
            (0..num_sets)
                .map(|_| self.get_grand_product_challenge_set(num_challenges))
                .collect()
        }
    }

    impl<F: RichField, H: Hasher<F>> GrandProductChallengeTrait<F, H> for Challenger<F, H> {
        fn get_grand_product_challenge(&mut self) -> GrandProductChallenge<F> {
            let beta = self.get_challenge();
            let gamma = self.get_challenge();
            GrandProductChallenge { beta, gamma }
        }
    }
}

/// A single instance of a permutation check protocol.
pub(crate) struct PermutationInstance<'a, T: Copy + Eq + PartialEq + Debug> {
    pub(crate) pair: &'a PermutationPair,
    pub(crate) challenge: GrandProductChallenge<T>,
}

/// Get a list of instances of our batch-permutation argument. These are
/// permutation arguments where the same `Z(x)` polynomial is used to check more
/// than one permutation. Before batching, each permutation pair leads to
/// `num_challenges` permutation arguments, so we start with the cartesian
/// product of `permutation_pairs` and `0..num_challenges`. Then we chunk these
/// arguments based on our batch size.
pub(crate) fn get_permutation_batches<'a, T: Copy + Eq + PartialEq + Debug>(
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

/// Compute Z(x) polynomials for all challenges, where each Z(x) polynomials is
/// for a given set of permutations, applied over a set of column pairs.
///
/// Targeted STARK  must explicitly override the permutation column when
/// implementing the [`Stark`] trait.
pub(crate) fn compute_permutation_z_polys<F, S, const D: usize>(
    stark: &S,
    config: &StarkConfig,
    trace_poly_values: &[PolynomialValues<F>],
    permutation_challenge_sets: &[GrandProductChallengeSet<F>],
) -> Vec<PolynomialValues<F>>
where
    F: RichField + Extendable<D>,
    S: Stark<F, D>, {
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

/// Compute a single Z(x) polynomial for a given set of permutations, each
/// applied over a set of column pairs.
///
/// To better understand the permutation polynomial, you can refer to the
/// section 2.5 Multi-set equality of the [eSTARK](https://eprint.iacr.org/2023/474.pdf) paper.
/// There, however, they do not batch permutations together, unlike here.
#[allow(clippy::similar_names)]
fn compute_permutation_z_poly<F: Field>(
    instances: &[PermutationInstance<F>],
    trace_poly_values: &[PolynomialValues<F>],
) -> PolynomialValues<F> {
    // All trace polynomials have the same degree (amount of rows).
    let degree = trace_poly_values[0].len();
    // Calculate the reduced polynomials, combining pairs of columns that abide
    // the same permutation.
    let (reduced_lhs_polys, reduced_rhs_polys): (Vec<_>, Vec<_>) = instances
        .iter()
        .map(|instance| permutation_reduced_polys(instance, trace_poly_values, degree))
        .unzip();

    // In order for the permutation to hold, `numerator / denominator == 1` must
    // hold.

    // The following should give an intuition behind this:
    // Fr each permutation `sigma`, we have generated two reduced polynomials, that
    // uniquely characterises each row of both left set of columns that
    // participate in the permutation, as well as right columns.
    //
    // Additionally, the way we have incorporated randomness when calculating the
    // reduced polynomials makes the reduced polynomial values semi-random.
    //
    // Now, if we multiply reduced polynomials values row by row, if the result is
    // equal on both left and right side, then with high probability all values on
    // the left and right hand side should also be equal. And the fact that the
    // values of reduced are semi-random removes the chance that the prover can
    // specifically craft two permutations that would annihilate each other.
    let numerator = poly_product_elementwise(reduced_lhs_polys.into_iter());
    let denominator = poly_product_elementwise(reduced_rhs_polys.into_iter());

    // Compute the quotients.
    let denominator_inverses = F::batch_multiplicative_inverse(&denominator.values);
    let mut quotients = numerator.values;
    batch_multiply_inplace(&mut quotients, &denominator_inverses);

    // Compute Z, which contains partial products of the quotients.
    // If indeed all the permutations between the left and right sides are correct,
    // then for each reduced polynomials value on left there is a row with the same
    // reduced polynomials value on the right. Which means if we take product of all
    // such values, then it should be 1.
    // This implies that `Z(degree - 1) == 1 <-> all permutations hold`
    let mut partial_products = Vec::with_capacity(degree);
    let mut acc = F::ONE;
    for q in quotients {
        partial_products.push(acc);
        acc *= q;
    }
    PolynomialValues::new(partial_products)
}

/// Computes the reduced polynomial by squashing all pairs of permutation
/// column, each represented by a polynomial, into a single pair of polynomials.
/// This is done by adding together the polynomial values with a secure random
/// `beta` and `gamma`. For this to work, the permutation should be identical
/// for all column pairs.
///
/// The following is how we calculate the reduced polynomial, for both "left"
/// and "right" sides:   `poly_reduced(x) = \sum beta^i poly_i(x) + gamma`
fn permutation_reduced_polys<F: Field>(
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

/// Computes the elementwise product of a set of polynomials. Assumes that the
/// set is non-empty and that each polynomial has the same length.
fn poly_product_elementwise<F: Field>(
    mut polys: impl Iterator<Item = PolynomialValues<F>>,
) -> PolynomialValues<F> {
    let mut product = polys.next().expect("Expected at least one polynomial");
    for poly in polys {
        batch_multiply_inplace(&mut product.values, &poly.values);
    }
    product
}

pub struct PermutationCheckVars<F, FE, P, const D2: usize>
where
    F: Field,
    FE: FieldExtension<D2, BaseField = F>,
    P: PackedField<Scalar = FE>, {
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
    S: Stark<F, D>, {
    let PermutationCheckVars {
        local_zs,
        next_zs,
        permutation_challenge_sets,
    } = permutation_vars;

    // Check that Z(1) = 1
    // This is how we should have initiated the Z(x) polynomial.
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

    // For each batch check that the permutation constraints indeed hold.
    for (i, instances) in permutation_batches.iter().enumerate() {
        // Calculate the reduced polynomials evaluation. The reduced polynomial securely
        // combines pairs of columns that abide the same permutation into one
        // polynomial pair.
        //
        // This differs from the [`permutation_reduced_polys`] function as instead of
        // working on polynomials, we are now operating on their evaluation at a
        // challenge point.
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
        // Check that Z(x) has been calculated correctly, that is:
        //  Z(gx) = Z(x) * \prod ( reduced_lhs_i(x) / reduced_rhs_i(x) )
        let constraint = next_zs[i] * reduced_rhs.into_iter().product::<P>()
            - local_zs[i] * reduced_lhs.into_iter().product::<P>();
        consumer.constraint(constraint);
    }
}

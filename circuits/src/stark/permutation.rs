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
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::iop::target::Target;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::config::Hasher;
use plonky2::plonk::plonk_common::reduce_with_powers;
use plonky2::util::reducing::{ReducingFactor, ReducingFactorTarget};
use rayon::prelude::{IntoParallelIterator, ParallelIterator};
use starky::config::StarkConfig;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};
use starky::evaluation_frame::StarkEvaluationFrame;
use starky::permutation::PermutationPair;
use starky::stark::Stark;

use crate::stark::permutation::challenge::{GrandProductChallenge, GrandProductChallengeSet};

pub(crate) mod challenge {
    use plonky2::field::extension::Extendable;
    use plonky2::iop::challenger::RecursiveChallenger;
    use plonky2::iop::ext_target::ExtensionTarget;
    use plonky2::iop::target::Target;
    use plonky2::plonk::circuit_builder::CircuitBuilder;
    use plonky2::plonk::config::AlgebraicHasher;
    use plonky2::plonk::plonk_common::{
        reduce_with_powers_circuit, reduce_with_powers_ext_circuit,
    };

    use super::{
        reduce_with_powers, Challenger, Debug, Field, FieldExtension, Hasher, PackedField,
        RichField,
    };

    /// Randomness parameters that the are used to generate a unique
    /// projection (mapping) of list of field elements.
    /// If two lists have the same projection values, then, with high
    /// probability, the two lists are identical. Though collisions can
    /// happen, our security depends on the low probability of such events.
    ///
    /// To reduce the probability of failure, one can rerun the protocol
    /// multiple times, each time generating new [`beta`] and [`gamma`]. We
    /// do this in [`GrandProductChallengeSet`].
    ///
    /// In the permutation check protocol instance we use this challenge to make
    /// sure that rows of two sets of columns are the same, up to permutation.
    #[derive(Copy, Clone, Eq, PartialEq, Debug)]
    pub(crate) struct GrandProductChallenge<T: Copy + Eq + PartialEq + Debug> {
        /// Randomness used to combine multiple columns into one.
        pub(crate) beta: T,
        /// Random offset that's added to the beta-reduced column values.
        pub(crate) gamma: T,
    }

    impl<F: Field> GrandProductChallenge<F> {
        /// Calculate the mapping for the list of values as:
        ///     `term_0*beta^n + term_1*beta^(n-1) + ... + term_n-1 + gamma`
        /// where `n` is the length of the list.
        ///
        /// ### Reasoning
        ///
        /// Let us consider a simpler unique projection first:
        ///      `term_0*beta_0 + term_1*beta_1 + ... + term_n-1 + gamma`
        /// where `beta_i` are random values.
        ///
        /// This mapping makes it very hard for the prover to find two lists
        /// that map to the same value. However, it requires `n` random values.
        /// By taking the power of `beta` we can reduce the number of random
        /// values to just two, `beta` and `gamma`. This is by using a fact that
        /// a power of a random value can be seen as an independent a
        /// random value. We still need to use `gamma` to make sure that the
        /// prover can  not manipulate the last list value in a way that would
        /// make the two lists equal.
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

    impl GrandProductChallenge<Target> {
        pub(crate) fn combine_circuit<F: RichField + Extendable<D>, const D: usize>(
            &self,
            builder: &mut CircuitBuilder<F, D>,
            terms: &[ExtensionTarget<D>],
        ) -> ExtensionTarget<D> {
            let reduced = reduce_with_powers_ext_circuit(builder, terms, self.beta);
            let gamma = builder.convert_to_ext(self.gamma);
            builder.add_extension(reduced, gamma)
        }
    }

    impl GrandProductChallenge<Target> {
        pub(crate) fn combine_base_circuit<F: RichField + Extendable<D>, const D: usize>(
            &self,
            builder: &mut CircuitBuilder<F, D>,
            terms: &[Target],
        ) -> Target {
            let reduced = reduce_with_powers_circuit(builder, terms, self.beta);
            builder.add(reduced, self.gamma)
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

    fn get_grand_product_challenge_target<
        F: RichField + Extendable<D>,
        H: AlgebraicHasher<F>,
        const D: usize,
    >(
        builder: &mut CircuitBuilder<F, D>,
        challenger: &mut RecursiveChallenger<F, H, D>,
    ) -> GrandProductChallenge<Target> {
        let beta = challenger.get_challenge(builder);
        let gamma = challenger.get_challenge(builder);
        GrandProductChallenge { beta, gamma }
    }

    pub(crate) fn get_grand_product_challenge_set_target<
        F: RichField + Extendable<D>,
        H: AlgebraicHasher<F>,
        const D: usize,
    >(
        builder: &mut CircuitBuilder<F, D>,
        challenger: &mut RecursiveChallenger<F, H, D>,
        num_challenges: usize,
    ) -> GrandProductChallengeSet<Target> {
        let challenges = (0..num_challenges)
            .map(|_| get_grand_product_challenge_target(builder, challenger))
            .collect();
        GrandProductChallengeSet { challenges }
    }

    pub(crate) fn get_n_grand_product_challenge_sets_target<
        F: RichField + Extendable<D>,
        H: AlgebraicHasher<F>,
        const D: usize,
    >(
        builder: &mut CircuitBuilder<F, D>,
        challenger: &mut RecursiveChallenger<F, H, D>,
        num_challenges: usize,
        num_sets: usize,
    ) -> Vec<GrandProductChallengeSet<Target>> {
        (0..num_sets)
            .map(|_| get_grand_product_challenge_set_target(builder, challenger, num_challenges))
            .collect()
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
///
/// The batch size is chosen to be the `constraint degree - 1`, as all resulting
/// `Z(x)` polynomials in the batch will be multiplied together.
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

/// Compute Z(x) polynomials for all challenges, where each Z(x) polynomial is
/// for a given set of permutations, applied over a set of column pairs.
///
/// Targeted STARK must explicitly override the permutation column when
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

    // In order for the permutation to match, `numerator / denominator == 1` must
    // hold.

    // The following should give an intuition behind this:
    // For each permutation, we have generated two reduced polynomials, that
    // uniquely characterise each row of both left set of columns that
    // participate in the permutation, as well as right set of columns.
    //
    // Additionally, the way we have incorporated randomness when calculating the
    // reduced polynomials makes the reduced polynomial values semi-random.
    //
    // Now, if we multiply reduced polynomials row by row, if the resulting
    // polynomials are equal, then with high probability all values on
    // the left and right hand side should also be equal. And the fact that the
    // reduced polynomials are semi-random minimizes the chance that the
    // prover can specifically craft two permutations that would annihilate each
    // other.
    let numerator = poly_product_elementwise(reduced_lhs_polys.into_iter());
    let denominator = poly_product_elementwise(reduced_rhs_polys.into_iter());

    // Compute the quotients.
    let denominator_inverses = F::batch_multiplicative_inverse(&denominator.values);
    let mut quotients = numerator.values;
    batch_multiply_inplace(&mut quotients, &denominator_inverses);

    // Compute Z, which contains partial products of the quotients.
    // If indeed all the permutations between the left and right sides are correct,
    // then if we take product of all reduced polynomials values, then it must be 1.
    // This implies that `Z(degree - 1) == 1 <-> all permutations hold`
    let mut partial_products = Vec::with_capacity(degree);
    let mut acc = F::ONE;
    for q in quotients {
        partial_products.push(acc);
        acc *= q;
    }
    PolynomialValues::new(partial_products)
}

/// Computes the reduced polynomial pair from a list of column permutation
/// pairs, stored in the [`PermutationInstance`].
///
/// The following is the formula of the reduced left/right polynomial:
///   `poly_reduced(x) = \sum beta^i poly_i(x) + gamma`
///
/// Where:
/// - `beta` is a secure random value
/// - `gamma` is a secure random value
/// - `poly_i(x)` is the left(right)  polynomial value of the `i`-th pair
/// - `poly_reduced(x)` is the reduced left(right) polynomial
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
    vars: &S::EvaluationFrame<FE, P, D2>,
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

    // Split the permutation pairs into batches to reduce the number of
    // constraints.
    let permutation_batches = get_permutation_batches(
        &permutation_pairs,
        &permutation_challenge_sets,
        config.num_challenges,
        stark.permutation_batch_size(),
    );

    // For each batch we check that the permutation constraints indeed hold.
    for (i, instances) in permutation_batches.iter().enumerate() {
        // Calculate the reduced polynomials evaluation. The reduced polynomial securely
        // combines polynomials of all pairs of `(column, permuted column)` that are on
        // the same permutation into one polynomial pair.
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
                    .map(|&(i, j)| (vars.get_local_values()[i], vars.get_local_values()[j]))
                    .unzip();
                (
                    factor.reduce_ext(lhs.into_iter()) + FE::from_basefield(*gamma),
                    factor.reduce_ext(rhs.into_iter()) + FE::from_basefield(*gamma),
                )
            })
            .unzip();
        // Check that Z(x) has been calculated correctly, that is:
        //  Z(gx) = Z(x) * \prod ( reduced_lhs_i(x) / reduced_rhs_i(x) )
        // For convenience, we have rearranged the equation to:
        //  Z(gx) * \prod ( reduced_rhs_i(x) ) - Z(x) *  \prod ( reduced_lhs_i(x) ) = 0
        let constraint = next_zs[i] * reduced_rhs.into_iter().product::<P>()
            - local_zs[i] * reduced_lhs.into_iter().product::<P>();
        consumer.constraint(constraint);
    }
}

pub struct PermutationCheckDataTarget<const D: usize> {
    pub(crate) local_zs: Vec<ExtensionTarget<D>>,
    pub(crate) next_zs: Vec<ExtensionTarget<D>>,
    pub(crate) permutation_challenge_sets: Vec<GrandProductChallengeSet<Target>>,
}

pub(crate) fn eval_permutation_checks_circuit<F, S, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    stark: &S,
    config: &StarkConfig,
    vars: &S::EvaluationFrameTarget,
    permutation_data: PermutationCheckDataTarget<D>,
    consumer: &mut RecursiveConstraintConsumer<F, D>,
) where
    F: RichField + Extendable<D>,
    S: Stark<F, D>, {
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
                        .map(|&(i, j)| (vars.get_local_values()[i], vars.get_local_values()[j]))
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

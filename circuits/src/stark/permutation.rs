//! This module handles logic behind the permutation arguments:
//! challenge generation, batching, constraint generation and verification.

#![allow(clippy::module_name_repetitions)]

use std::fmt::Debug;

use plonky2::field::extension::FieldExtension;
use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::challenger::Challenger;
use plonky2::plonk::config::Hasher;
use plonky2::plonk::plonk_common::reduce_with_powers;

pub mod challenge {
    use plonky2::field::extension::Extendable;
    use plonky2::iop::challenger::RecursiveChallenger;
    use plonky2::iop::ext_target::ExtensionTarget;
    use plonky2::iop::target::Target;
    use plonky2::plonk::circuit_builder::CircuitBuilder;
    use plonky2::plonk::config::AlgebraicHasher;
    use plonky2::plonk::plonk_common::reduce_with_powers_ext_circuit;

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
    pub struct GrandProductChallenge<T: Copy + Eq + PartialEq + Debug> {
        /// Randomness used to combine multiple columns into one.
        pub beta: T,
        /// Random offset that's added to the beta-reduced column values.
        pub gamma: T,
    }

    impl<F: Field> GrandProductChallenge<F> {
        /// Calculate the mapping for the list of values as:
        ///     `term_0*beta^{n-1} + term_1*beta^{n-2} + ... + term_n-1 + gamma`
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
        pub fn combine<'a, FE, P, T: IntoIterator<Item = &'a P>, const D2: usize>(
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
        pub fn combine_circuit<F: RichField + Extendable<D>, const D: usize>(
            &self,
            builder: &mut CircuitBuilder<F, D>,
            terms: &[ExtensionTarget<D>],
        ) -> ExtensionTarget<D> {
            let reduced = reduce_with_powers_ext_circuit(builder, terms, self.beta);
            let gamma = builder.convert_to_ext(self.gamma);
            builder.add_extension(reduced, gamma)
        }
    }

    impl<Target> From<GrandProductChallenge<Target>> for starky::lookup::GrandProductChallenge<Target>
    where
        Target: Copy + Eq + PartialEq + Debug,
    {
        fn from(challenge: GrandProductChallenge<Target>) -> Self {
            starky::lookup::GrandProductChallenge {
                beta: challenge.beta,
                gamma: challenge.gamma,
            }
        }
    }

    /// [`GrandProductChallenge`] repeated for [`num_challenges`] to boost
    /// soundness.
    #[derive(Clone, Eq, PartialEq, Debug, Default)]
    pub struct GrandProductChallengeSet<T: Copy + Eq + PartialEq + core::fmt::Debug> {
        pub challenges: Vec<GrandProductChallenge<T>>,
    }

    impl<Target> From<GrandProductChallengeSet<Target>>
        for starky::lookup::GrandProductChallengeSet<Target>
    where
        Target: Copy + Eq + PartialEq + core::fmt::Debug,
    {
        fn from(challenges: GrandProductChallengeSet<Target>) -> Self {
            starky::lookup::GrandProductChallengeSet {
                challenges: challenges
                    .challenges
                    .into_iter()
                    .map(starky::lookup::GrandProductChallenge::from)
                    .collect(),
            }
        }
    }

    pub trait GrandProductChallengeTrait<F: RichField, H: Hasher<F>> {
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
    }

    impl<F: RichField, H: Hasher<F>> GrandProductChallengeTrait<F, H> for Challenger<F, H> {
        fn get_grand_product_challenge(&mut self) -> GrandProductChallenge<F> {
            let beta = self.get_challenge();
            let gamma = self.get_challenge();
            GrandProductChallenge { beta, gamma }
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

    /// Circuit version of `get_grand_product_challenge_set`.
    pub fn get_grand_product_challenge_set_target<
        F: RichField + Extendable<D>,
        H: AlgebraicHasher<F>,
        const D: usize,
    >(
        builder: &mut CircuitBuilder<F, D>,
        recursive_challenger: &mut RecursiveChallenger<F, H, D>,
        num_challenges: usize,
    ) -> GrandProductChallengeSet<Target> {
        let challenges = (0..num_challenges)
            .map(|_| get_grand_product_challenge_target(builder, recursive_challenger))
            .collect();
        GrandProductChallengeSet { challenges }
    }
}

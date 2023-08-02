use itertools::Itertools;
use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::fri::oracle::PolynomialBatch;
use plonky2::fri::proof::{FriChallenges, FriProof};
use plonky2::fri::structure::{FriOpeningBatch, FriOpenings};
use plonky2::hash::hash_types::RichField;
use plonky2::hash::merkle_tree::MerkleCap;
use plonky2::iop::challenger::Challenger;
use plonky2::plonk::config::GenericConfig;
use plonky2_maybe_rayon::{MaybeParIter, ParallelIterator};
use serde::{Deserialize, Serialize};
use starky::config::StarkConfig;

use super::mozak_stark::{MozakStark, NUM_TABLES};
use super::permutation::{
    get_grand_product_challenge_set, get_n_grand_product_challenge_sets, GrandProductChallengeSet,
};

#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(bound = "")]
pub struct StarkProof<F: RichField + Extendable<D>, C: GenericConfig<D, F = F>, const D: usize> {
    /// Merkle cap of LDEs of trace values.
    pub trace_cap: MerkleCap<F, C::Hasher>,
    /// Merkle cap of LDEs of permutation Z values.
    pub permutation_ctl_zs_cap: MerkleCap<F, C::Hasher>,
    /// Merkle cap of LDEs of trace values.
    pub quotient_polys_cap: MerkleCap<F, C::Hasher>,
    /// Purported values of each polynomial at the challenge point.
    pub openings: StarkOpeningSet<F, D>,
    /// A batch FRI argument for all openings.
    pub opening_proof: FriProof<F, C::Hasher, D>,
}

impl<F: RichField + Extendable<D>, C: GenericConfig<D, F = F>, const D: usize> StarkProof<F, C, D> {
    /// Recover the length of the trace from a STARK proof and a STARK config.
    pub fn recover_degree_bits(&self, config: &StarkConfig) -> usize {
        let initial_merkle_proof = &self.opening_proof.query_round_proofs[0]
            .initial_trees_proof
            .evals_proofs[0]
            .1;
        let lde_bits = config.fri_config.cap_height + initial_merkle_proof.siblings.len();
        lde_bits - config.fri_config.rate_bits
    }

    pub fn num_ctl_zs(&self) -> usize { self.openings.ctl_zs_last.len() }

    /// Computes all Fiat-Shamir challenges used in the STARK proof.
    pub(crate) fn get_challenges(
        &self,
        challenger: &mut Challenger<F, C::Hasher>,
        stark_permutation_batch_size: usize,
        config: &StarkConfig,
    ) -> StarkProofChallenges<F, D> {
        let degree_bits = self.recover_degree_bits(config);

        let StarkProof {
            permutation_ctl_zs_cap,
            quotient_polys_cap,
            openings,
            opening_proof:
                FriProof {
                    commit_phase_merkle_caps,
                    final_poly,
                    pow_witness,
                    ..
                },
            ..
        } = &self;

        let num_challenges = config.num_challenges;

        let permutation_challenge_sets = get_n_grand_product_challenge_sets(
            challenger,
            num_challenges,
            stark_permutation_batch_size,
        );

        challenger.observe_cap(permutation_ctl_zs_cap);

        let stark_alphas = challenger.get_n_challenges(num_challenges);

        challenger.observe_cap(quotient_polys_cap);
        let stark_zeta = challenger.get_extension_challenge::<D>();

        challenger.observe_openings(&openings.to_fri_openings());

        StarkProofChallenges {
            permutation_challenge_sets,
            stark_alphas,
            stark_zeta,
            fri_challenges: challenger.fri_challenges::<C, D>(
                commit_phase_merkle_caps,
                final_poly,
                *pow_witness,
                degree_bits,
                &config.fri_config,
            ),
        }
    }
}

pub(crate) struct StarkProofChallenges<F: RichField + Extendable<D>, const D: usize> {
    /// Randomness used in any permutation arguments.
    pub permutation_challenge_sets: Vec<GrandProductChallengeSet<F>>,

    /// Random values used to combine STARK constraints.
    pub stark_alphas: Vec<F>,

    /// Point at which the STARK polynomials are opened.
    pub stark_zeta: F::Extension,

    pub fri_challenges: FriChallenges<F, D>,
}

/// Purported values of each polynomial at the challenge point.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(bound = "")]
pub struct StarkOpeningSet<F: RichField + Extendable<D>, const D: usize> {
    /// Openings of trace polynomials at `zeta`.
    pub local_values: Vec<F::Extension>,
    /// Openings of trace polynomials at `g * zeta`.
    pub next_values: Vec<F::Extension>,
    /// Openings of permutations and cross-table lookups `Z` polynomials at
    /// `zeta`.
    pub permutation_ctl_zs: Vec<F::Extension>,
    /// Openings of permutations and cross-table lookups `Z` polynomials at `g *
    /// zeta`.
    pub permutation_ctl_zs_next: Vec<F::Extension>,
    /// Openings of cross-table lookups `Z` polynomials at `g^-1`.
    pub ctl_zs_last: Vec<F>,
    /// Openings of quotient polynomials at `zeta`.
    pub quotient_polys: Vec<F::Extension>,
}

impl<F: RichField + Extendable<D>, const D: usize> StarkOpeningSet<F, D> {
    pub fn new<C: GenericConfig<D, F = F>>(
        zeta: F::Extension,
        g: F,
        trace_commitment: &PolynomialBatch<F, C, D>,
        permutation_ctl_zs_commitment: &PolynomialBatch<F, C, D>,
        quotient_commitment: &PolynomialBatch<F, C, D>,
        degree_bits: usize,
        num_permutation_zs: usize,
    ) -> Self {
        let eval_commitment = |z: F::Extension, c: &PolynomialBatch<F, C, D>| {
            c.polynomials
                .par_iter()
                .map(|p| p.to_extension().eval(z))
                .collect::<Vec<_>>()
        };
        let eval_commitment_base = |z: F, c: &PolynomialBatch<F, C, D>| {
            c.polynomials
                .par_iter()
                .map(|p| p.eval(z))
                .collect::<Vec<_>>()
        };
        let zeta_next = zeta.scalar_mul(g);
        Self {
            local_values: eval_commitment(zeta, trace_commitment),
            next_values: eval_commitment(zeta_next, trace_commitment),
            permutation_ctl_zs: eval_commitment(zeta, permutation_ctl_zs_commitment),
            permutation_ctl_zs_next: eval_commitment(zeta_next, permutation_ctl_zs_commitment),
            ctl_zs_last: eval_commitment_base(
                F::primitive_root_of_unity(degree_bits).inverse(),
                permutation_ctl_zs_commitment,
            )[num_permutation_zs..]
                .to_vec(),
            quotient_polys: eval_commitment(zeta, quotient_commitment),
        }
    }

    pub(crate) fn to_fri_openings(&self) -> FriOpenings<F, D> {
        let zeta_batch = FriOpeningBatch {
            values: self
                .local_values
                .iter()
                .chain(&self.permutation_ctl_zs)
                .chain(&self.quotient_polys)
                .copied()
                .collect_vec(),
        };
        let zeta_next_batch = FriOpeningBatch {
            values: self
                .next_values
                .iter()
                .chain(&self.permutation_ctl_zs_next)
                .copied()
                .collect_vec(),
        };
        debug_assert!(!self.ctl_zs_last.is_empty());
        let ctl_last_batch = FriOpeningBatch {
            values: self
                .ctl_zs_last
                .iter()
                .copied()
                .map(F::Extension::from_basefield)
                .collect(),
        };

        FriOpenings {
            batches: vec![zeta_batch, zeta_next_batch, ctl_last_batch],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(bound = "")]
#[allow(clippy::module_name_repetitions)]
pub struct AllProof<F: RichField + Extendable<D>, C: GenericConfig<D, F = F>, const D: usize> {
    pub stark_proofs: [StarkProof<F, C, D>; NUM_TABLES],
}

pub(crate) struct AllProofChallenges<F: RichField + Extendable<D>, const D: usize> {
    pub stark_challenges: [StarkProofChallenges<F, D>; NUM_TABLES],
    pub ctl_challenges: GrandProductChallengeSet<F>,
}

impl<F: RichField + Extendable<D>, C: GenericConfig<D, F = F>, const D: usize> AllProof<F, C, D> {
    /// Computes all Fiat-Shamir challenges used in the STARK proof.
    pub(crate) fn get_challenges(
        &self,
        all_stark: &MozakStark<F, D>,
        config: &StarkConfig,
    ) -> AllProofChallenges<F, D> {
        let mut challenger = Challenger::<F, C::Hasher>::new();

        for proof in &self.stark_proofs {
            challenger.observe_cap(&proof.trace_cap);
        }

        // TODO: Observe public values.

        let ctl_challenges =
            get_grand_product_challenge_set(&mut challenger, config.num_challenges);

        let num_permutation_batch_sizes = all_stark.permutation_batch_sizes();

        AllProofChallenges {
            stark_challenges: core::array::from_fn(|i| {
                challenger.compact();
                self.stark_proofs[i].get_challenges(
                    &mut challenger,
                    num_permutation_batch_sizes[i],
                    config,
                )
            }),
            ctl_challenges,
        }
    }

    /// Returns the ordered openings of cross-table lookups `Z` polynomials at
    /// `g^-1`. The order corresponds to the order declared in
    /// [`TableKind`](crate::cross_table_lookup::TableKind).
    pub(crate) fn all_ctl_zs_last(self) -> [Vec<F>; NUM_TABLES] {
        self.stark_proofs.map(|p| p.openings.ctl_zs_last)
    }
}

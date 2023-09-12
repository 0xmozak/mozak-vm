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

use super::mozak_stark::NUM_TABLES;
use crate::lookup::{self, Lookup};
use crate::stark::mozak_stark::PublicInputs;
use crate::stark::permutation::challenge::{GrandProductChallengeSet, GrandProductChallengeTrait};

#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(bound = "")]
pub struct StarkProof<F: RichField + Extendable<D>, C: GenericConfig<D, F = F>, const D: usize> {
    /// Merkle cap of LDEs of trace values.
    pub trace_cap: MerkleCap<F, C::Hasher>,
    /// Merkle cap of LDEs of permutation Z values.
    pub auxiliary_polys_cap: MerkleCap<F, C::Hasher>,
    /// Merkle cap of LDEs of trace values.
    pub quotient_polys_cap: MerkleCap<F, C::Hasher>,
    /// Purported values of each polynomial at the challenge point.
    pub openings: StarkOpeningSet<F, D>,
    /// A batch FRI argument for all openings.
    pub opening_proof: FriProof<F, C::Hasher, D>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(bound = "")]
pub struct StarkProofWithLookups<
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    const D: usize,
> {
    pub proof: StarkProof<F, C, D>,
    pub lookups: Option<Vec<Lookup>>,
}

impl<F: RichField + Extendable<D>, C: GenericConfig<D, F = F>, const D: usize>
    StarkProofWithLookups<F, C, D>
{
    /// Computes all Fiat-Shamir challenges used in the STARK proof.
    pub(crate) fn get_challenges(
        &self,
        challenger: &mut Challenger<F, C::Hasher>,
        config: &StarkConfig,
    ) -> StarkProofChallenges<F, D> {
        let StarkProofWithLookups { lookups, proof } = &self;
        let degree_bits = proof.recover_degree_bits(config);

        let num_challenges = config.num_challenges;

        let lookup_challenges = lookups
            .as_ref()
            .map(|_| challenger.get_n_challenges(config.num_challenges));

        challenger.observe_cap(&proof.auxiliary_polys_cap);

        let stark_alphas = challenger.get_n_challenges(num_challenges);

        challenger.observe_cap(&proof.quotient_polys_cap);
        let stark_zeta = challenger.get_extension_challenge::<D>();

        challenger.observe_openings(&proof.openings.to_fri_openings());

        StarkProofChallenges {
            lookup_challenges,
            stark_alphas,
            stark_zeta,
            fri_challenges: challenger.fri_challenges::<C, D>(
                &proof.opening_proof.commit_phase_merkle_caps,
                &proof.opening_proof.final_poly,
                proof.opening_proof.pow_witness,
                degree_bits,
                &config.fri_config,
            ),
        }
    }

    pub(crate) fn num_helper_columns(&self, config: &StarkConfig) -> usize {
        self.lookups.as_ref().map_or(0, |ls| {
            ls.iter()
                .map(lookup::Lookup::num_helper_columns)
                .sum::<usize>()
                * config.num_challenges
        })
    }
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
}

pub(crate) struct StarkProofChallenges<F: RichField + Extendable<D>, const D: usize> {
    /// Randomness used in lookup arguments.
    pub lookup_challenges: Option<Vec<F>>,

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
    /// Openings of lookups and cross-table lookups `Z` polynomials at `zeta`.
    pub auxiliary_polys: Vec<F::Extension>,
    /// Openings of lookups and cross-table lookups `Z` polynomials at `g *
    /// zeta`.
    pub auxiliary_polys_next: Vec<F::Extension>,
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
        auxiliary_polys_commitment: &PolynomialBatch<F, C, D>,
        quotient_commitment: &PolynomialBatch<F, C, D>,
        degree_bits: usize,
        num_lookup_columns: usize,
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
            auxiliary_polys: eval_commitment(zeta, auxiliary_polys_commitment),
            auxiliary_polys_next: eval_commitment(zeta_next, auxiliary_polys_commitment),
            ctl_zs_last: eval_commitment_base(
                F::primitive_root_of_unity(degree_bits).inverse(),
                auxiliary_polys_commitment,
            )[num_lookup_columns..]
                .to_vec(),
            quotient_polys: eval_commitment(zeta, quotient_commitment),
        }
    }

    pub(crate) fn to_fri_openings(&self) -> FriOpenings<F, D> {
        let zeta_batch = FriOpeningBatch {
            values: self
                .local_values
                .iter()
                .chain(&self.auxiliary_polys)
                .chain(&self.quotient_polys)
                .copied()
                .collect_vec(),
        };
        let zeta_next_batch = FriOpeningBatch {
            values: self
                .next_values
                .iter()
                .chain(&self.auxiliary_polys_next)
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
    pub stark_proofs: [StarkProofWithLookups<F, C, D>; NUM_TABLES],
    pub program_rom_trace_cap: MerkleCap<F, C::Hasher>,
    pub public_inputs: PublicInputs<F>,
}

pub(crate) struct AllProofChallenges<F: RichField + Extendable<D>, const D: usize> {
    pub stark_challenges: [StarkProofChallenges<F, D>; NUM_TABLES],
    pub ctl_challenges: GrandProductChallengeSet<F>,
}

impl<F: RichField + Extendable<D>, C: GenericConfig<D, F = F>, const D: usize> AllProof<F, C, D> {
    /// Computes all Fiat-Shamir challenges used in the STARK proof.
    pub(crate) fn get_challenges(&self, config: &StarkConfig) -> AllProofChallenges<F, D> {
        let mut challenger = Challenger::<F, C::Hasher>::new();

        for proof in &self.stark_proofs {
            challenger.observe_cap(&proof.proof.trace_cap);
        }

        // TODO: Observe public values.

        let ctl_challenges = challenger.get_grand_product_challenge_set(config.num_challenges);

        AllProofChallenges {
            stark_challenges: core::array::from_fn(|i| {
                challenger.compact();
                self.stark_proofs[i].get_challenges(&mut challenger, config)
            }),
            ctl_challenges,
        }
    }

    /// Returns the ordered openings of cross-table lookups `Z` polynomials at
    /// `g^-1`. The order corresponds to the order declared in
    /// [`TableKind`](crate::cross_table_lookup::TableKind).
    pub(crate) fn all_ctl_zs_last(self) -> [Vec<F>; NUM_TABLES] {
        self.stark_proofs.map(|p| p.proof.openings.ctl_zs_last)
    }
}

use itertools::{chain, Itertools};
use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::fri::oracle::PolynomialBatch;
use plonky2::fri::proof::{FriChallenges, FriChallengesTarget, FriProof, FriProofTarget};
use plonky2::fri::structure::{
    FriOpeningBatch, FriOpeningBatchTarget, FriOpenings, FriOpeningsTarget,
};
use plonky2::hash::hash_types::{MerkleCapTarget, RichField};
use plonky2::hash::merkle_tree::MerkleCap;
use plonky2::iop::challenger::{Challenger, RecursiveChallenger};
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::iop::target::Target;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::config::{AlgebraicHasher, GenericConfig};
#[allow(clippy::wildcard_imports)]
use plonky2_maybe_rayon::*;
use serde::{Deserialize, Serialize};
use starky::config::StarkConfig;

use super::mozak_stark::{all_kind, PublicInputs, TableKindArray};
use crate::public_sub_table::PublicSubTableValues;
use crate::stark::permutation::challenge::{GrandProductChallengeSet, GrandProductChallengeTrait};

#[allow(clippy::module_name_repetitions)]
impl<F: RichField + Extendable<D>, C: GenericConfig<D, F = F>, const D: usize> AllProof<F, C, D> {
    pub fn degree_bits(&self, config: &StarkConfig) -> TableKindArray<usize> {
        all_kind!(|kind| self.proofs[kind].recover_degree_bits(config))
    }
}

#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(bound = "")]
pub struct StarkProof<F: RichField + Extendable<D>, C: GenericConfig<D, F = F>, const D: usize> {
    /// Merkle cap of LDEs of trace values.
    pub trace_cap: MerkleCap<F, C::Hasher>,
    /// Merkle cap of LDEs of cross-table lookup Z values.
    pub ctl_zs_cap: MerkleCap<F, C::Hasher>,
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
        config: &StarkConfig,
    ) -> StarkProofChallenges<F, D> {
        let degree_bits = self.recover_degree_bits(config);

        let StarkProof {
            ctl_zs_cap,
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

        challenger.observe_cap(ctl_zs_cap);

        let stark_alphas = challenger.get_n_challenges(num_challenges);

        challenger.observe_cap(quotient_polys_cap);
        let stark_zeta = challenger.get_extension_challenge::<D>();

        challenger.observe_openings(&openings.to_fri_openings());

        StarkProofChallenges {
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StarkProofTarget<const D: usize> {
    pub trace_cap: MerkleCapTarget,
    pub ctl_zs_cap: MerkleCapTarget,
    pub quotient_polys_cap: MerkleCapTarget,
    pub openings: StarkOpeningSetTarget<D>,
    pub opening_proof: FriProofTarget<D>,
}

impl<const D: usize> StarkProofTarget<D> {
    #[must_use]
    /// Recover the length of the trace from a STARK proof and a STARK config.
    pub fn recover_degree_bits(&self, config: &StarkConfig) -> usize {
        let initial_merkle_proof = &self.opening_proof.query_round_proofs[0]
            .initial_trees_proof
            .evals_proofs[0]
            .1;
        let lde_bits = config.fri_config.cap_height + initial_merkle_proof.siblings.len();
        lde_bits - config.fri_config.rate_bits
    }
}

impl<const D: usize> StarkProofTarget<D> {
    pub fn get_challenges<F: RichField + Extendable<D>, C: GenericConfig<D, F = F>>(
        &self,
        builder: &mut CircuitBuilder<F, D>,
        challenger: &mut RecursiveChallenger<F, C::Hasher, D>,
        config: &StarkConfig,
    ) -> StarkProofChallengesTarget<D>
    where
        C::Hasher: AlgebraicHasher<F>, {
        let StarkProofTarget {
            ctl_zs_cap,
            quotient_polys_cap,
            openings,
            opening_proof:
                FriProofTarget {
                    commit_phase_merkle_caps,
                    final_poly,
                    pow_witness,
                    ..
                },
            ..
        } = &self;

        let num_challenges = config.num_challenges;

        challenger.observe_cap(ctl_zs_cap);

        let stark_alphas = challenger.get_n_challenges(builder, num_challenges);

        challenger.observe_cap(quotient_polys_cap);
        let stark_zeta = challenger.get_extension_challenge(builder);

        challenger.observe_openings(&openings.to_fri_openings(builder.zero()));

        StarkProofChallengesTarget {
            stark_alphas,
            stark_zeta,
            fri_challenges: challenger.fri_challenges(
                builder,
                commit_phase_merkle_caps,
                final_poly,
                *pow_witness,
                &config.fri_config,
            ),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StarkProofWithPublicInputsTarget<const D: usize> {
    pub proof: StarkProofTarget<D>,
    pub public_inputs: Vec<Target>,
}

pub struct StarkProofChallenges<F: RichField + Extendable<D>, const D: usize> {
    /// Random values used to combine STARK constraints.
    pub stark_alphas: Vec<F>,

    /// Point at which the STARK polynomials are opened.
    pub stark_zeta: F::Extension,

    pub fri_challenges: FriChallenges<F, D>,
}

pub struct StarkProofChallengesTarget<const D: usize> {
    pub stark_alphas: Vec<Target>,
    pub stark_zeta: ExtensionTarget<D>,
    pub fri_challenges: FriChallengesTarget<D>,
}

/// Purported values of each polynomial at the challenge point.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(bound = "")]
pub struct StarkOpeningSet<F: RichField + Extendable<D>, const D: usize> {
    /// Openings of trace polynomials at `zeta`.
    pub local_values: Vec<F::Extension>,
    /// Openings of trace polynomials at `g * zeta`.
    pub next_values: Vec<F::Extension>,
    /// Openings of cross-table lookups `Z` polynomials at
    /// `zeta`.
    pub ctl_zs: Vec<F::Extension>,
    /// Openings of cross-table lookups `Z` polynomials at `g *
    /// zeta`.
    pub ctl_zs_next: Vec<F::Extension>,
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
        ctl_zs_commitment: &PolynomialBatch<F, C, D>,
        quotient_commitment: &PolynomialBatch<F, C, D>,
        degree_bits: usize,
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
            ctl_zs: eval_commitment(zeta, ctl_zs_commitment),
            ctl_zs_next: eval_commitment(zeta_next, ctl_zs_commitment),
            ctl_zs_last: eval_commitment_base(
                F::primitive_root_of_unity(degree_bits).inverse(),
                ctl_zs_commitment,
            ),
            quotient_polys: eval_commitment(zeta, quotient_commitment),
        }
    }

    pub(crate) fn to_fri_openings(&self) -> FriOpenings<F, D> {
        let zeta_batch = FriOpeningBatch {
            values: chain!(&self.local_values, &self.ctl_zs, &self.quotient_polys)
                .copied()
                .collect_vec(),
        };
        let zeta_next_batch = FriOpeningBatch {
            values: chain!(&self.next_values, &self.ctl_zs_next,)
                .copied()
                .collect_vec(),
        };
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StarkOpeningSetTarget<const D: usize> {
    pub local_values: Vec<ExtensionTarget<D>>,
    pub next_values: Vec<ExtensionTarget<D>>,
    pub ctl_zs: Vec<ExtensionTarget<D>>,
    pub ctl_zs_next: Vec<ExtensionTarget<D>>,
    pub ctl_zs_last: Vec<Target>,
    pub quotient_polys: Vec<ExtensionTarget<D>>,
}

impl<const D: usize> StarkOpeningSetTarget<D> {
    pub(crate) fn to_fri_openings(&self, zero: Target) -> FriOpeningsTarget<D> {
        let zeta_batch = FriOpeningBatchTarget {
            values: chain!(&self.local_values, &self.ctl_zs, &self.quotient_polys)
                .copied()
                .collect_vec(),
        };
        let zeta_next_batch = FriOpeningBatchTarget {
            values: chain!(&self.next_values, &self.ctl_zs_next)
                .copied()
                .collect_vec(),
        };
        debug_assert!(!self.ctl_zs_last.is_empty());
        let ctl_last_batch = FriOpeningBatchTarget {
            values: self
                .ctl_zs_last
                .iter()
                .map(|t| t.to_ext_target(zero))
                .collect(),
        };

        FriOpeningsTarget {
            batches: vec![zeta_batch, zeta_next_batch, ctl_last_batch],
        }
    }
}

#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(bound = "")]
pub struct AllProof<F: RichField + Extendable<D>, C: GenericConfig<D, F = F>, const D: usize> {
    pub proofs: TableKindArray<StarkProof<F, C, D>>,
    pub program_rom_trace_cap: MerkleCap<F, C::Hasher>,
    pub elf_memory_init_trace_cap: MerkleCap<F, C::Hasher>,
    pub public_inputs: PublicInputs<F>,
    pub public_sub_table_values: TableKindArray<Vec<PublicSubTableValues<F>>>,
}

#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(bound = "")]
pub struct BatchProof<F: RichField + Extendable<D>, C: GenericConfig<D, F = F>, const D: usize> {
    pub degree_bits: TableKindArray<usize>,
    pub proofs: TableKindArray<StarkProof<F, C, D>>,
    pub program_rom_trace_cap: MerkleCap<F, C::Hasher>,
    pub elf_memory_init_trace_cap: MerkleCap<F, C::Hasher>,
    pub public_inputs: PublicInputs<F>,
    pub public_sub_table_values: TableKindArray<Vec<PublicSubTableValues<F>>>,
    pub batch_stark_proof: StarkProof<F, C, D>,
}

pub(crate) struct AllProofChallenges<F: RichField + Extendable<D>, const D: usize> {
    pub stark_challenges: TableKindArray<StarkProofChallenges<F, D>>,
    pub ctl_challenges: GrandProductChallengeSet<F>,
}

impl<F: RichField + Extendable<D>, C: GenericConfig<D, F = F>, const D: usize> AllProof<F, C, D> {
    /// Computes all Fiat-Shamir challenges used in the STARK proof.
    pub(crate) fn get_challenges(&self, config: &StarkConfig) -> AllProofChallenges<F, D> {
        let mut challenger = Challenger::<F, C::Hasher>::new();

        for proof in &self.proofs {
            challenger.observe_cap(&proof.trace_cap);
        }

        // TODO: Observe public values.

        let ctl_challenges = challenger.get_grand_product_challenge_set(config.num_challenges);

        AllProofChallenges {
            stark_challenges: all_kind!(|kind| {
                challenger.compact();
                self.proofs[kind].get_challenges(&mut challenger, config)
            }),
            ctl_challenges,
        }
    }

    /// Returns the ordered openings of cross-table lookups `Z` polynomials at
    /// `g^-1`. The order corresponds to the order declared in
    /// [`TableKind`](crate::cross_table_lookup::TableKind).
    pub(crate) fn all_ctl_zs_last(self) -> TableKindArray<Vec<F>> {
        self.proofs.map(|p| p.openings.ctl_zs_last)
    }
}

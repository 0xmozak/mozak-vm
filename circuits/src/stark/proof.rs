use itertools::Itertools;
use plonky2::fri::oracle::PolynomialBatch;
use plonky2::fri::structure::{FriOpeningBatch, FriOpenings};
use plonky2::{
    field::extension::{Extendable, FieldExtension},
    fri::proof::FriProof,
    hash::{hash_types::RichField, merkle_tree::MerkleCap},
    plonk::config::GenericConfig,
};
use plonky2_maybe_rayon::*;

use super::mozak_stark::NUM_TABLES;

#[derive(Debug, Clone)]
pub struct StarkProof<F: RichField + Extendable<D>, C: GenericConfig<D, F = F>, const D: usize> {
    /// Merkle cap of LDEs of trace values.
    pub trace_cap: MerkleCap<F, C::Hasher>,
    /// Merkle cap of LDEs of quotient polynomials values.
    pub quotient_polys_cap: MerkleCap<F, C::Hasher>,
    /// Purported values of each polynomials at the challenge point.
    pub openings: StarkOpeningSet<F, D>,
    /// A batch FRI argument for all openings.
    pub opening_proof: FriProof<F, C::Hasher, D>,
}

#[derive(Debug, Clone)]
pub struct StarkOpeningSet<F: RichField + Extendable<D>, const D: usize> {
    /// Openings of trace polynomials at `zeta`.
    pub local_values: Vec<F::Extension>,
    /// Openings of trace polynomials at `g * zeta`.
    pub next_values: Vec<F::Extension>,
    /// Openings of quotient polynomials at `zeta`.
    pub quotient_polys: Vec<F::Extension>,
}

#[derive(Debug, Clone)]
pub struct AllProof<F: RichField + Extendable<D>, C: GenericConfig<D, F = F>, const D: usize> {
    pub stark_proofs: [StarkProof<F, C, D>; NUM_TABLES],
    pub compress_challenges: [F; NUM_TABLES],
}

impl<F: RichField + Extendable<D>, const D: usize> StarkOpeningSet<F, D> {
    pub fn new<C: GenericConfig<D, F = F>>(
        zeta: F::Extension,
        g: F,
        trace_commitment: &PolynomialBatch<F, C, D>,
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
            quotient_polys: eval_commitment(zeta, quotient_commitment),
        }
    }
    pub(crate) fn to_fri_openings(&self) -> FriOpenings<F, D> {
        let zeta_batch = FriOpeningBatch {
            values: self
                .local_values
                .iter()
                .chain(&self.quotient_polys)
                .copied()
                .collect_vec(),
        };
        let zeta_next_batch = FriOpeningBatch {
            values: self.next_values.iter().copied().collect_vec(),
        };

        FriOpenings {
            batches: vec![zeta_batch, zeta_next_batch],
        }
    }
}

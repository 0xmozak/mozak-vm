use plonky2::{
    field::extension::Extendable,
    fri::proof::FriProof,
    hash::{hash_types::RichField, merkle_tree::MerkleCap},
    plonk::config::GenericConfig,
};

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

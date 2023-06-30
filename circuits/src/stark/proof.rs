use plonky2::{
    field::extension::Extendable, hash::hash_types::RichField, plonk::config::GenericConfig,
};
use starky::proof::StarkProof;

use super::mozak_stark::NUM_TABLES;

/// A `StarkProof` along with some metadata about the initial Fiat-Shamir state,
/// which is used when creating a recursive wrapper proof around a STARK proof.
#[derive(Debug, Clone)]
pub struct StarkProofWithMetadata<F, C, const D: usize>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
{
    pub(crate) proof: StarkProof<F, C, D>,
}

#[derive(Debug, Clone)]
#[allow(clippy::module_name_repetitions)]
pub struct AllProof<F: RichField + Extendable<D>, C: GenericConfig<D, F = F>, const D: usize> {
    pub stark_proofs: [StarkProof<F, C, D>; NUM_TABLES],
}

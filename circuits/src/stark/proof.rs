use plonky2::{
    field::extension::Extendable, hash::hash_types::RichField, plonk::config::GenericConfig,
};
use starky::proof::StarkProofWithPublicInputs;

use super::mozak_stark::NUM_TABLES;

#[derive(Debug, Clone)]
#[allow(clippy::module_name_repetitions)]
pub struct AllProof<F: RichField + Extendable<D>, C: GenericConfig<D, F = F>, const D: usize> {
    pub stark_proofs: [StarkProofWithPublicInputs<F, C, D>; NUM_TABLES],
}

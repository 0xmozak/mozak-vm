use anyhow::Result;
use plonky2::plonk::config::Hasher;
use plonky2::{
    field::extension::Extendable, hash::hash_types::RichField, plonk::config::GenericConfig,
};
use starky::proof::StarkProofWithPublicInputs;
use starky::stark::Stark;
use starky::{config::StarkConfig, verifier::verify_stark_proof};

use super::{mozak_stark::MozakStark, proof::AllProof};
use crate::cpu::stark::CpuStark;

#[allow(clippy::missing_errors_doc)]
pub fn verify_proof<F, C, const D: usize>(
    mozak_stark: &MozakStark<F, D>,
    all_proof: &AllProof<F, C, D>,
    config: &StarkConfig,
) -> Result<()>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    [(); CpuStark::<F, D>::COLUMNS]:,
    [(); CpuStark::<F, D>::PUBLIC_INPUTS]:,
    [(); C::Hasher::HASH_SIZE]:,
{
    let MozakStark { cpu_stark, .. } = mozak_stark;

    let cpu_proof = StarkProofWithPublicInputs {
        proof: all_proof.stark_proofs[0].clone(),
        public_inputs: Vec::new(),
    };
    verify_stark_proof(*cpu_stark, cpu_proof, config)
}

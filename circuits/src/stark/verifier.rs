use anyhow::Result;
use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::RichField;
use plonky2::plonk::config::{GenericConfig, Hasher};
use starky::config::StarkConfig;
use starky::stark::Stark;
use starky::verifier::verify_stark_proof;

use super::mozak_stark::MozakStark;
use super::proof::AllProof;
use crate::bitwise::stark::BitwiseStark;
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
    [(); BitwiseStark::<F, D>::COLUMNS]:,
    [(); BitwiseStark::<F, D>::PUBLIC_INPUTS]:,
    [(); CpuStark::<F, D>::COLUMNS]:,
    [(); CpuStark::<F, D>::PUBLIC_INPUTS]:,
    [(); C::Hasher::HASH_SIZE]:, {
    let MozakStark {
        cpu_stark,
        bitwise_stark,
    } = mozak_stark;
    verify_stark_proof(*cpu_stark, all_proof.stark_proofs[0].clone(), config)?;
    verify_stark_proof(*bitwise_stark, all_proof.stark_proofs[1].clone(), config)
}

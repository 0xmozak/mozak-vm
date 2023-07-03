use anyhow::Result;
use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::RichField;
use plonky2::plonk::config::{GenericConfig, Hasher};
use starky::config::StarkConfig;
use starky::proof::StarkProofWithPublicInputs;
use starky::stark::Stark;
use starky::verifier::verify_stark_proof;

use super::mozak_stark::MozakStark;
use super::proof::AllProof;
// use crate::bitwise;
use crate::bitwise::stark::BitwiseStark;
use crate::cpu::stark::CpuStark;
use crate::stark::mozak_stark::NUM_TABLES;

#[allow(clippy::missing_errors_doc)]
pub fn verify_proof_cpu<F, C, const D: usize>(
    mozak_stark: MozakStark<F, D>,
    all_proof: AllProof<F, C, D>,
    config: &StarkConfig,
) -> Result<()>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    [(); CpuStark::<F, D>::PUBLIC_INPUTS]:,
    [(); CpuStark::<F, D>::COLUMNS]:,
    [(); C::Hasher::HASH_SIZE]:, {
    let MozakStark {
        cpu_stark,
        bitwise_stark: _,
    } = mozak_stark;
    let [cpu_proof, _]: [StarkProofWithPublicInputs<F, C, D>; NUM_TABLES] = all_proof.stark_proofs;

    verify_stark_proof::<F, C, CpuStark<F, D>, D>(cpu_stark, cpu_proof, config)
}

#[allow(clippy::missing_errors_doc)]
pub fn verify_proof_bitwise<F, C, const D: usize>(
    mozak_stark: MozakStark<F, D>,
    all_proof: AllProof<F, C, D>,
    config: &StarkConfig,
) -> Result<()>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    [(); BitwiseStark::<F, D>::PUBLIC_INPUTS]:,
    [(); BitwiseStark::<F, D>::COLUMNS]:,
    [(); C::Hasher::HASH_SIZE]:, {
    let MozakStark {
        cpu_stark: _,
        bitwise_stark,
    } = mozak_stark;
    let [_, bitwise_proof]: [StarkProofWithPublicInputs<F, C, D>; NUM_TABLES] =
        all_proof.stark_proofs;

    verify_stark_proof::<F, C, BitwiseStark<F, D>, D>(bitwise_stark, bitwise_proof, config)
}

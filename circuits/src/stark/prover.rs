use anyhow::Result;
use mozak_vm::vm::Row;
use plonky2::field::extension::Extendable;
use plonky2::field::polynomial::PolynomialValues;
use plonky2::hash::hash_types::RichField;
use plonky2::plonk::config::GenericConfig;
use plonky2::plonk::config::Hasher;
use plonky2::util::timing::TimingTree;
use starky::config::StarkConfig;
use starky::prover::prove as prove_table;
use starky::stark::Stark;

use super::mozak_stark::{MozakStark, NUM_TABLES};
use super::proof::AllProof;
use crate::cpu::cpu_stark::CpuStark;
use crate::generation::generate_traces;

pub fn prove<F, C, const D: usize>(
    step_rows: Vec<Row>,
    mozak_stark: &mut MozakStark<F, D>,
    config: &StarkConfig,
    timing: &mut TimingTree,
) -> Result<AllProof<F, C, D>>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    [(); CpuStark::<F, D>::COLUMNS]:,
    [(); CpuStark::<F, D>::PUBLIC_INPUTS]:,
    [(); C::Hasher::HASH_SIZE]:,
{
    let trace_poly_values = generate_traces(step_rows);
    prove_with_traces(mozak_stark, config, trace_poly_values, timing)
}

pub fn prove_with_traces<F, C, const D: usize>(
    mozak_stark: &MozakStark<F, D>,
    config: &StarkConfig,
    trace_poly_values: [Vec<PolynomialValues<F>>; NUM_TABLES],
    timing: &mut TimingTree,
) -> Result<AllProof<F, C, D>>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    [(); CpuStark::<F, D>::COLUMNS]:,
    [(); CpuStark::<F, D>::PUBLIC_INPUTS]:,
    [(); C::Hasher::HASH_SIZE]:,
{
    let cpu_proof = prove_table(
        mozak_stark.cpu_stark,
        config,
        trace_poly_values[0].clone(),
        [],
        timing,
    )?;
    let stark_proofs = [cpu_proof];

    let compress_challenges = [mozak_stark.cpu_stark.get_compress_challenge().unwrap()];

    Ok(AllProof {
        stark_proofs,
        compress_challenges,
    })
}

#[cfg(test)]
mod test {
    use mozak_vm::test_utils::simple_test;

    #[test]
    fn prove_add() {}
}

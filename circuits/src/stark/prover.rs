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
use crate::cpu::stark::CpuStark;
use crate::generation::generate_traces;

#[allow(clippy::missing_errors_doc)]
pub fn prove<F, C, const D: usize>(
    step_rows: &[Row],
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
    prove_with_traces(mozak_stark, config, &trace_poly_values, timing)
}

#[allow(clippy::missing_errors_doc)]
pub fn prove_with_traces<F, C, const D: usize>(
    mozak_stark: &MozakStark<F, D>,
    config: &StarkConfig,
    trace_poly_values: &[Vec<PolynomialValues<F>>; NUM_TABLES],
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

    Ok(AllProof { stark_proofs })
}

#[cfg(test)]
mod test {
    use mozak_vm::test_utils::simple_test;
    use plonky2::{
        plonk::config::{GenericConfig, PoseidonGoldilocksConfig},
        util::timing::TimingTree,
    };
    use starky::config::StarkConfig;

    use super::prove;
    use crate::stark::mozak_stark::MozakStark;
    use crate::stark::verifier::verify_proof;

    #[test]
    fn prove_halt() {
        const D: usize = 2;
        type C = PoseidonGoldilocksConfig;
        type F = <C as GenericConfig<D>>::F;
        type S = MozakStark<F, D>;
        let (rows, _state) = simple_test(0, &[], &[]);
        let mut config = StarkConfig::standard_fast_config();
        config.fri_config.cap_height = 0;

        let mut stark = S::default();
        let proof = prove::<F, C, D>(&rows, &mut stark, &config, &mut TimingTree::default());
        assert!(proof.is_ok());
    }

    #[test]
    fn prove_add() {
        const D: usize = 2;
        type C = PoseidonGoldilocksConfig;
        type F = <C as GenericConfig<D>>::F;
        type S = MozakStark<F, D>;
        let (rows, state) = simple_test(
            4,
            &[(0_u32, 0x0073_02b3 /* add r5, r6, r7 */)],
            &[(6, 100), (7, 100)],
        );
        assert_eq!(state.get_register_value(5), 100 + 100);
        let mut config = StarkConfig::standard_fast_config();
        config.fri_config.cap_height = 0;

        let mut stark = S::default();
        let all_proof = prove::<F, C, D>(&rows, &mut stark, &config, &mut TimingTree::default());
        assert!(all_proof.is_ok());
        let res = verify_proof(&stark, &all_proof.unwrap(), &config);
        assert!(res.is_ok());
    }
}

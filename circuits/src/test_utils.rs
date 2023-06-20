use anyhow::Result;
use mozak_vm::vm::Row;
use plonky2::{
    plonk::config::{GenericConfig, PoseidonGoldilocksConfig},
    util::timing::TimingTree,
};
use starky::config::StarkConfig;

use crate::stark::mozak_stark::MozakStark;
use crate::stark::prover::prove;
use crate::stark::verifier::verify_proof;

#[allow(clippy::missing_panics_doc)]
#[allow(clippy::missing_errors_doc)]
pub fn simple_proof_test(step_rows: &[Row]) -> Result<()> {
    const D: usize = 2;
    type C = PoseidonGoldilocksConfig;
    type F = <C as GenericConfig<D>>::F;
    type S = MozakStark<F, D>;
    let mut config = StarkConfig::standard_fast_config();
    config.fri_config.cap_height = 0;

    let mut stark = S::default();
    let all_proof = prove::<F, C, D>(step_rows, &mut stark, &config, &mut TimingTree::default());
    verify_proof(&stark, &all_proof.unwrap(), &config)
}

use anyhow::Result;
use mozak_vm::vm::Row;
use plonky2::fri::FriConfig;
use plonky2::plonk::config::{GenericConfig, PoseidonGoldilocksConfig};
use plonky2::util::log2_ceil;
use plonky2::util::timing::TimingTree;
use starky::config::StarkConfig;
use starky::stark::Stark;

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
    let mut stark = S::default();
    let config = StarkConfig::standard_fast_config();
    let config = StarkConfig {
        security_bits: 1,
        num_challenges: 2,
        fri_config: FriConfig {
            // Plonky2 says: "Having constraints of degree higher than the rate is not supported
            // yet." So we automatically set the rate here as required by plonky2.
            rate_bits: log2_ceil(stark.cpu_stark.constraint_degree()),
            cap_height: 0,
            proof_of_work_bits: 0,
            ..config.fri_config
        },
    };

    let all_proof = prove::<F, C, D>(step_rows, &mut stark, &config, &mut TimingTree::default());
    verify_proof(stark, &all_proof.unwrap(), &config)
}

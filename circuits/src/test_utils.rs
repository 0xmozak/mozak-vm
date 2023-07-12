use anyhow::Result;
use mozak_vm::vm::Row;
use plonky2::fri::FriConfig;
use plonky2::hash::hash_types::RichField;
use plonky2::plonk::config::{GenericConfig, PoseidonGoldilocksConfig};
use plonky2::util::log2_ceil;
use plonky2::util::timing::TimingTree;
use starky::config::StarkConfig;
use starky::stark::Stark;

use crate::stark::mozak_stark::MozakStark;
use crate::stark::prover::prove;
use crate::stark::verifier::verify_proof;

pub const D: usize = 2;
pub type C = PoseidonGoldilocksConfig;
pub type F = <C as GenericConfig<D>>::F;
pub type S = MozakStark<F, D>;

#[must_use]
pub fn standard_faster_config() -> StarkConfig {
    let config = StarkConfig::standard_fast_config();
    StarkConfig {
        security_bits: 1,
        num_challenges: 2,
        fri_config: FriConfig {
            // Plonky2 says: "Having constraints of degree higher than the rate is not supported
            // yet." So we automatically set the rate here as required by plonky2.
            // TODO(Matthias): Change to maximum of constraint degrees of all starks, as we
            // accumulate more types of starks.
            rate_bits: log2_ceil(S::default().cpu_stark.constraint_degree()),
            cap_height: 0,
            proof_of_work_bits: 0,
            num_query_rounds: 5,
            ..config.fri_config
        },
    }
}

#[allow(clippy::missing_panics_doc)]
#[allow(clippy::missing_errors_doc)]
pub fn simple_proof_test(step_rows: &[Row]) -> Result<()> {
    let mut stark = S::default();
    let config = standard_faster_config();

    let all_proof = prove::<F, C, D>(step_rows, &mut stark, &config, &mut TimingTree::default());
    verify_proof(&stark, &all_proof.unwrap(), &config)
}

/// Interpret a u64 as a field element and try to invert it.
///
/// Internally, we are doing something like: inv(a) == a^(p-2)
/// Specifically that means inv(0) == 0, and inv(a) * a == 1 for everything
/// else.
#[must_use]
pub fn inv<F: RichField>(x: u64) -> u64 {
    F::from_canonical_u64(x)
        .try_inverse()
        .unwrap_or_default()
        .to_canonical_u64()
}

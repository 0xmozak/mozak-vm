use anyhow::Result;
use mozak_vm::vm::Row;
use plonky2::fri::FriConfig;
use plonky2::hash::hash_types::RichField;
use plonky2::plonk::config::{GenericConfig, PoseidonGoldilocksConfig};
use plonky2::util::log2_ceil;
use plonky2::util::timing::TimingTree;
use starky::config::StarkConfig;
use starky::prover::prove as prove_table;
use starky::stark::Stark;
use starky::verifier::verify_stark_proof;

use crate::bitwise::stark::BitwiseStark;
use crate::cpu::stark::CpuStark;
use crate::generation::bitshift::generate_shift_amount_trace;
use crate::generation::bitwise::generate_bitwise_trace;
use crate::generation::cpu::generate_cpu_trace;
use crate::generation::memory::generate_memory_trace;
use crate::generation::rangecheck::generate_rangecheck_trace;
use crate::memory::stark::MemoryStark;
use crate::rangecheck::stark::RangeCheckStark;
use crate::shift_amount::stark::ShiftAmountStark;
use crate::stark::mozak_stark::MozakStark;
use crate::stark::prover::prove;
use crate::stark::utils::{trace_rows_to_poly_values, trace_to_poly_values};
use crate::stark::verifier::verify_proof;

pub type S = MozakStark<F, D>;
pub const D: usize = 2;
pub type C = PoseidonGoldilocksConfig;
pub type F = <C as GenericConfig<D>>::F;

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

pub trait ProveAndVerify {
    /// Prove and verify a [`Stark`].
    ///
    /// Depending on the implementation this verifies either a single STARK,
    /// or a [`MozakStark`]. Proving and verifying a single STARK will be
    /// faster, but does not include cross table lookups; proving and verifying
    /// a [`MozakStark`] will prove and verify all STARKs and include cross
    /// table lookups, but will be much more expensive.
    ///
    /// # Errors
    /// Errors if proving or verifying the STARK fails.
    fn prove_and_verify(step_rows: &[Row]) -> Result<()>;
}

impl ProveAndVerify for CpuStark<F, D> {
    fn prove_and_verify(step_rows: &[Row]) -> Result<()> {
        type S = CpuStark<F, D>;

        let config = standard_faster_config();

        let stark = S::default();
        let trace_poly_values = trace_rows_to_poly_values(generate_cpu_trace(step_rows));
        let proof = prove_table::<F, C, S, D>(
            stark,
            &config,
            trace_poly_values,
            [],
            &mut TimingTree::default(),
        )?;

        verify_stark_proof(stark, proof, &config)
    }
}

impl ProveAndVerify for RangeCheckStark<F, D> {
    fn prove_and_verify(step_rows: &[Row]) -> Result<()> {
        type S = RangeCheckStark<F, D>;

        let config = standard_faster_config();

        let stark = S::default();
        let cpu_trace = generate_cpu_trace(step_rows);
        let trace_poly_values = trace_to_poly_values(generate_rangecheck_trace(&cpu_trace));
        let proof = prove_table::<F, C, S, D>(
            stark,
            &config,
            trace_poly_values,
            [],
            &mut TimingTree::default(),
        )?;

        verify_stark_proof(stark, proof, &config)
    }
}

impl ProveAndVerify for BitwiseStark<F, D> {
    fn prove_and_verify(step_rows: &[Row]) -> Result<()> {
        type S = BitwiseStark<F, D>;

        let config = standard_faster_config();

        let stark = S::default();
        let cpu_trace = generate_cpu_trace(step_rows);
        let trace_poly_values = trace_rows_to_poly_values(generate_bitwise_trace(&cpu_trace));
        let proof = prove_table::<F, C, S, D>(
            stark,
            &config,
            trace_poly_values,
            [],
            &mut TimingTree::default(),
        )?;

        verify_stark_proof(stark, proof, &config)
    }
}

impl ProveAndVerify for MemoryStark<F, D> {
    fn prove_and_verify(step_rows: &[Row]) -> Result<()> {
        type S = MemoryStark<F, D>;
        let config = standard_faster_config();

        let stark = S::default();
        let trace_poly_values = trace_rows_to_poly_values(generate_memory_trace(step_rows));
        let proof = prove_table::<F, C, S, D>(
            stark,
            &config,
            trace_poly_values,
            [],
            &mut TimingTree::default(),
        )?;

        verify_stark_proof(stark, proof, &config)
    }
}

impl ProveAndVerify for ShiftAmountStark<F, D> {
    fn prove_and_verify(step_rows: &[Row]) -> Result<()> {
        type S = ShiftAmountStark<F, D>;
        let config = standard_faster_config();

        let stark = S::default();
        let cpu_rows = generate_cpu_trace::<F>(step_rows);
        let trace = generate_shift_amount_trace(&cpu_rows);
        let trace_poly_values = trace_rows_to_poly_values(trace);
        let proof = prove_table::<F, C, S, D>(
            stark,
            &config,
            trace_poly_values,
            [],
            &mut TimingTree::default(),
        )?;

        verify_stark_proof(stark, proof, &config)
    }
}

impl ProveAndVerify for MozakStark<F, D> {
    /// Prove and verify a [`MozakStark`].
    ///
    /// Note that this variant is a lot slower than the others, because
    /// this proves and verifies ALL starks and lookups within the Mozak
    /// ZKVM. This should be preferred if the test is concerned with the
    /// consistency of the final [`MozakStark`].
    fn prove_and_verify(step_rows: &[Row]) -> Result<()> {
        let stark = S::default();
        let config = standard_faster_config();

        let all_proof = prove::<F, C, D>(step_rows, &stark, &config, &mut TimingTree::default());
        verify_proof(stark, all_proof.unwrap(), &config)
    }
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

use std::borrow::Borrow;

use anyhow::Result;
use mozak_runner::elf::Program;
use mozak_runner::vm::ExecutionRecord;
use plonky2::fri::FriConfig;
use plonky2::hash::hash_types::RichField;
use plonky2::plonk::config::{GenericConfig, Poseidon2GoldilocksConfig};
use plonky2::util::log2_ceil;
use plonky2::util::timing::TimingTree;
use starky::config::StarkConfig;
use starky::prover::prove as prove_table;
use starky::stark::Stark;
use starky::verifier::verify_stark_proof;

use crate::bitshift::stark::BitshiftStark;
use crate::cpu::stark::CpuStark;
use crate::generation::bitshift::generate_shift_amount_trace;
use crate::generation::cpu::{generate_cpu_trace, generate_cpu_trace_extended};
use crate::generation::fullword_memory::generate_fullword_memory_trace;
use crate::generation::halfword_memory::generate_halfword_memory_trace;
use crate::generation::io_memory::{
    generate_io_memory_private_trace, generate_io_memory_public_trace,
};
use crate::generation::memory::generate_memory_trace;
use crate::generation::memoryinit::generate_memory_init_trace;
use crate::generation::program::generate_program_rom_trace;
use crate::generation::rangecheck::generate_rangecheck_trace;
use crate::generation::register::generate_register_trace;
use crate::generation::registerinit::generate_register_init_trace;
use crate::generation::xor::generate_xor_trace;
use crate::memory::stark::MemoryStark;
use crate::memory_fullword::stark::FullWordMemoryStark;
use crate::memory_halfword::stark::HalfWordMemoryStark;
use crate::memory_io::stark::InputOuputMemoryStark;
use crate::rangecheck::stark::RangeCheckStark;
use crate::register::stark::RegisterStark;
use crate::registerinit::stark::RegisterInitStark;
use crate::stark::mozak_stark::{MozakStark, PublicInputs};
use crate::stark::prover::prove;
use crate::stark::utils::{trace_rows_to_poly_values, trace_to_poly_values};
use crate::stark::verifier::verify_proof;
use crate::utils::from_u32;
use crate::xor::stark::XorStark;

pub type S = MozakStark<F, D>;
pub const D: usize = 2;
pub type C = Poseidon2GoldilocksConfig;
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

/// Prepares a table of a trace. Useful for trace generation tests.
#[must_use]
pub fn prep_table<F: RichField, T, const N: usize>(table: Vec<[u64; N]>) -> Vec<T>
where
    T: FromIterator<F>, {
    table
        .into_iter()
        .map(|row| row.into_iter().map(F::from_canonical_u64).collect())
        .collect()
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
    fn prove_and_verify(program: &Program, record: &ExecutionRecord<F>) -> Result<()>;
}

impl ProveAndVerify for CpuStark<F, D> {
    fn prove_and_verify(program: &Program, record: &ExecutionRecord<F>) -> Result<()> {
        type S = CpuStark<F, D>;

        let config = standard_faster_config();

        let stark = S::default();
        let trace_poly_values = trace_to_poly_values(generate_cpu_trace_extended(
            generate_cpu_trace(program, record),
            &generate_program_rom_trace(program),
        ));
        let public_inputs: PublicInputs<F> = PublicInputs {
            entry_point: from_u32(program.entry_point),
        };
        let proof = prove_table::<F, C, S, D>(
            stark,
            &config,
            trace_poly_values,
            public_inputs.borrow(),
            &mut TimingTree::default(),
        )?;

        verify_stark_proof(stark, proof, &config)
    }
}

impl ProveAndVerify for RangeCheckStark<F, D> {
    fn prove_and_verify(program: &Program, record: &ExecutionRecord<F>) -> Result<()> {
        type S = RangeCheckStark<F, D>;

        let config = standard_faster_config();

        let stark = S::default();
        let cpu_trace = generate_cpu_trace(program, record);
        let memory_init = generate_memory_init_trace(program);
        let halfword_memory = generate_halfword_memory_trace(program, &record.executed);
        let fullword_memory = generate_fullword_memory_trace(program, &record.executed);
        let io_memory_private = generate_io_memory_private_trace(program, &record.executed);
        let io_memory_public = generate_io_memory_public_trace(program, &record.executed);
        let memory_trace = generate_memory_trace::<F>(
            program,
            &record.executed,
            &memory_init,
            &halfword_memory,
            &fullword_memory,
            &io_memory_private,
            &io_memory_public,
        );
        let trace_poly_values =
            trace_rows_to_poly_values(generate_rangecheck_trace(&cpu_trace, &memory_trace));
        let proof = prove_table::<F, C, S, D>(
            stark,
            &config,
            trace_poly_values,
            &[],
            &mut TimingTree::default(),
        )?;

        verify_stark_proof(stark, proof, &config)
    }
}

impl ProveAndVerify for XorStark<F, D> {
    fn prove_and_verify(program: &Program, record: &ExecutionRecord<F>) -> Result<()> {
        type S = XorStark<F, D>;

        let config = standard_faster_config();

        let stark = S::default();
        let cpu_trace = generate_cpu_trace(program, record);
        let trace_poly_values = trace_rows_to_poly_values(generate_xor_trace(&cpu_trace));
        let proof = prove_table::<F, C, S, D>(
            stark,
            &config,
            trace_poly_values,
            &[],
            &mut TimingTree::default(),
        )?;

        verify_stark_proof(stark, proof, &config)
    }
}

impl ProveAndVerify for MemoryStark<F, D> {
    fn prove_and_verify(program: &Program, record: &ExecutionRecord<F>) -> Result<()> {
        type S = MemoryStark<F, D>;
        let config = standard_faster_config();

        let stark = S::default();
        let memory_init = generate_memory_init_trace(program);
        let halfword_memory = generate_halfword_memory_trace(program, &record.executed);
        let fullword_memory = generate_fullword_memory_trace(program, &record.executed);
        let io_memory_private = generate_io_memory_private_trace(program, &record.executed);
        let io_memory_public = generate_io_memory_public_trace(program, &record.executed);
        let trace_poly_values = trace_rows_to_poly_values(generate_memory_trace(
            program,
            &record.executed,
            &memory_init,
            &halfword_memory,
            &fullword_memory,
            &io_memory_private,
            &io_memory_public,
        ));
        let proof = prove_table::<F, C, S, D>(
            stark,
            &config,
            trace_poly_values,
            &[],
            &mut TimingTree::default(),
        )?;

        verify_stark_proof(stark, proof, &config)
    }
}

impl ProveAndVerify for HalfWordMemoryStark<F, D> {
    fn prove_and_verify(program: &Program, record: &ExecutionRecord<F>) -> Result<()> {
        type S = HalfWordMemoryStark<F, D>;
        let config = standard_faster_config();

        let stark = S::default();
        let trace_poly_values =
            trace_rows_to_poly_values(generate_halfword_memory_trace(program, &record.executed));
        let proof = prove_table::<F, C, S, D>(
            stark,
            &config,
            trace_poly_values,
            &[],
            &mut TimingTree::default(),
        )?;

        verify_stark_proof(stark, proof, &config)
    }
}

impl ProveAndVerify for FullWordMemoryStark<F, D> {
    fn prove_and_verify(program: &Program, record: &ExecutionRecord<F>) -> Result<()> {
        type S = FullWordMemoryStark<F, D>;
        let config = standard_faster_config();

        let stark = S::default();
        let trace_poly_values =
            trace_rows_to_poly_values(generate_fullword_memory_trace(program, &record.executed));
        let proof = prove_table::<F, C, S, D>(
            stark,
            &config,
            trace_poly_values,
            &[],
            &mut TimingTree::default(),
        )?;

        verify_stark_proof(stark, proof, &config)
    }
}

impl ProveAndVerify for InputOuputMemoryStark<F, D> {
    fn prove_and_verify(program: &Program, record: &ExecutionRecord<F>) -> Result<()> {
        type S = InputOuputMemoryStark<F, D>;
        let config = standard_faster_config();

        let stark = S::default();
        let trace_poly_values =
            trace_rows_to_poly_values(generate_io_memory_private_trace(program, &record.executed));
        let proof = prove_table::<F, C, S, D>(
            stark,
            &config,
            trace_poly_values,
            &[],
            &mut TimingTree::default(),
        )?;

        verify_stark_proof(stark, proof, &config)
    }
}

impl ProveAndVerify for BitshiftStark<F, D> {
    fn prove_and_verify(program: &Program, record: &ExecutionRecord<F>) -> Result<()> {
        type S = BitshiftStark<F, D>;
        let config = standard_faster_config();

        let stark = S::default();
        let cpu_rows = generate_cpu_trace::<F>(program, record);
        let trace = generate_shift_amount_trace(&cpu_rows);
        let trace_poly_values = trace_rows_to_poly_values(trace);
        let proof = prove_table::<F, C, S, D>(
            stark,
            &config,
            trace_poly_values,
            &[],
            &mut TimingTree::default(),
        )?;

        verify_stark_proof(stark, proof, &config)
    }
}

impl ProveAndVerify for RegisterInitStark<F, D> {
    fn prove_and_verify(_program: &Program, _record: &ExecutionRecord<F>) -> Result<()> {
        type S = RegisterInitStark<F, D>;
        let config = standard_faster_config();

        let stark = S::default();
        let trace = generate_register_init_trace::<F>();
        let trace_poly_values = trace_rows_to_poly_values(trace);
        let proof = prove_table::<F, C, S, D>(
            stark,
            &config,
            trace_poly_values,
            &[],
            &mut TimingTree::default(),
        )?;

        verify_stark_proof(stark, proof, &config)
    }
}

impl ProveAndVerify for RegisterStark<F, D> {
    fn prove_and_verify(program: &Program, record: &ExecutionRecord<F>) -> Result<()> {
        type S = RegisterStark<F, D>;
        let config = standard_faster_config();

        let stark = S::default();
        let trace = generate_register_trace::<F>(program, record);
        let trace_poly_values = trace_rows_to_poly_values(trace);
        let proof = prove_table::<F, C, S, D>(
            stark,
            &config,
            trace_poly_values,
            &[],
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
    fn prove_and_verify(program: &Program, record: &ExecutionRecord<F>) -> Result<()> {
        let stark = S::default();
        let config = standard_faster_config();
        let public_inputs = PublicInputs {
            entry_point: from_u32(program.entry_point),
        };

        let all_proof = prove::<F, C, D>(
            program,
            record,
            &stark,
            &config,
            public_inputs,
            &mut TimingTree::default(),
        );
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

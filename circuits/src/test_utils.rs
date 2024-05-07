use anyhow::Result;
use itertools::izip;
use mozak_runner::code;
use mozak_runner::decode::ECALL;
use mozak_runner::elf::Program;
use mozak_runner::instruction::{Args, Instruction, Op};
use mozak_runner::vm::ExecutionRecord;
use mozak_sdk::core::ecall;
use mozak_sdk::core::reg_abi::{REG_A0, REG_A1, REG_A2, REG_A3};
use plonky2::field::goldilocks_field::GoldilocksField;
use plonky2::field::types::Field;
use plonky2::fri::FriConfig;
use plonky2::hash::hash_types::{HashOut, RichField};
use plonky2::hash::poseidon2::Poseidon2Hash;
use plonky2::plonk::circuit_data::CircuitConfig;
use plonky2::plonk::config::{GenericConfig, Hasher, Poseidon2GoldilocksConfig};
use plonky2::util::log2_ceil;
use plonky2::util::timing::TimingTree;
use starky::config::StarkConfig;
use starky::prover::prove as prove_table;
use starky::stark::Stark;
use starky::verifier::verify_stark_proof;

use crate::bitshift::stark::BitshiftStark;
use crate::cpu::stark::CpuStark;
use crate::generation::bitshift::generate_shift_amount_trace;
use crate::generation::cpu::generate_cpu_trace;
use crate::generation::fullword_memory::generate_fullword_memory_trace;
use crate::generation::halfword_memory::generate_halfword_memory_trace;
use crate::generation::memory::generate_memory_trace;
use crate::generation::memory_zeroinit::generate_memory_zero_init_trace;
use crate::generation::memoryinit::generate_memory_init_trace;
use crate::generation::storage_device::{
    generate_call_tape_trace, generate_cast_list_commitment_tape_trace, generate_event_tape_trace,
    generate_events_commitment_tape_trace, generate_private_tape_trace, generate_public_tape_trace,
};
use crate::generation::xor::generate_xor_trace;
use crate::memory::stark::MemoryStark;
use crate::memory_fullword::stark::FullWordMemoryStark;
use crate::memory_halfword::stark::HalfWordMemoryStark;
use crate::ops;
use crate::poseidon2_output_bytes::generation::generate_poseidon2_output_bytes_trace;
use crate::poseidon2_sponge::generation::generate_poseidon2_sponge_trace;
use crate::rangecheck::generation::generate_rangecheck_trace;
use crate::rangecheck::stark::RangeCheckStark;
use crate::register::general::stark::RegisterStark;
use crate::register::generation::{generate_register_init_trace, generate_register_trace};
use crate::register::init::stark::RegisterInitStark;
use crate::stark::mozak_stark::{MozakStark, PublicInputs};
use crate::stark::prover::prove;
use crate::stark::utils::trace_rows_to_poly_values;
use crate::stark::verifier::verify_proof;
use crate::storage_device::stark::StorageDeviceStark;
use crate::tape_commitments::generation::generate_tape_commitments_trace;
use crate::tape_commitments::stark::TapeCommitmentsStark;
use crate::utils::from_u32;
use crate::xor::stark::XorStark;

pub type S = MozakStark<F, D>;
pub const D: usize = 2;
pub type C = Poseidon2GoldilocksConfig;
pub type F = <C as GenericConfig<D>>::F;

/// Test Configuration with 1 bit of security
#[must_use]
pub fn fast_test_config() -> StarkConfig {
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

#[must_use]
pub const fn fast_test_circuit_config() -> CircuitConfig {
    let mut config = CircuitConfig::standard_recursion_config();
    config.security_bits = 1;
    config.num_challenges = 1;
    config.fri_config.cap_height = 0;
    config.fri_config.proof_of_work_bits = 0;
    config.fri_config.num_query_rounds = 1;
    config
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
    fn prove_and_verify(_program: &Program, record: &ExecutionRecord<F>) -> Result<()> {
        type S = CpuStark<F, D>;

        let config = fast_test_config();

        let stark = S::default();
        let trace_poly_values = trace_rows_to_poly_values(generate_cpu_trace(record));
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

impl ProveAndVerify for RangeCheckStark<F, D> {
    fn prove_and_verify(program: &Program, record: &ExecutionRecord<F>) -> Result<()> {
        type S = RangeCheckStark<F, D>;

        let config = fast_test_config();

        let stark = S::default();
        let cpu_trace = generate_cpu_trace(record);
        let add_trace = ops::add::generate(record);
        let blt_trace = ops::blt_taken::generate(record);

        let memory_init = generate_memory_init_trace(program);
        let memory_zeroinit_rows = generate_memory_zero_init_trace(&record.executed, program);

        let halfword_memory = generate_halfword_memory_trace(&record.executed);
        let fullword_memory = generate_fullword_memory_trace(&record.executed);
        let private_tape = generate_private_tape_trace(&record.executed);
        let public_tape = generate_public_tape_trace(&record.executed);
        let call_tape_rows = generate_call_tape_trace(&record.executed);
        let event_tape_rows = generate_event_tape_trace(&record.executed);
        let events_commitment_tape_rows = generate_events_commitment_tape_trace(&record.executed);
        let cast_list_commitment_tape_rows =
            generate_cast_list_commitment_tape_trace(&record.executed);
        let poseidon2_sponge_trace = generate_poseidon2_sponge_trace(&record.executed);
        let poseidon2_output_bytes = generate_poseidon2_output_bytes_trace(&poseidon2_sponge_trace);
        let memory_trace = generate_memory_trace::<F>(
            &record.executed,
            &memory_init,
            &memory_zeroinit_rows,
            &halfword_memory,
            &fullword_memory,
            &private_tape,
            &public_tape,
            &call_tape_rows,
            &event_tape_rows,
            &events_commitment_tape_rows,
            &cast_list_commitment_tape_rows,
            &poseidon2_sponge_trace,
            &poseidon2_output_bytes,
        );
        let register_init = generate_register_init_trace(record);
        let (_, _, register_trace) = generate_register_trace(
            &cpu_trace,
            &add_trace,
            &blt_trace,
            &poseidon2_sponge_trace,
            &private_tape,
            &public_tape,
            &call_tape_rows,
            &event_tape_rows,
            &events_commitment_tape_rows,
            &cast_list_commitment_tape_rows,
            &register_init,
        );
        let trace_poly_values = trace_rows_to_poly_values(generate_rangecheck_trace(
            &cpu_trace,
            &add_trace,
            &blt_trace,
            &memory_trace,
            &register_trace,
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

impl ProveAndVerify for XorStark<F, D> {
    fn prove_and_verify(_program: &Program, record: &ExecutionRecord<F>) -> Result<()> {
        type S = XorStark<F, D>;

        let config = fast_test_config();

        let stark = S::default();
        let cpu_trace = generate_cpu_trace(record);
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
        let config = fast_test_config();

        let stark = S::default();

        let memory_init = generate_memory_init_trace(program);
        let memory_zeroinit_rows = generate_memory_zero_init_trace(&record.executed, program);

        let halfword_memory = generate_halfword_memory_trace(&record.executed);
        let fullword_memory = generate_fullword_memory_trace(&record.executed);
        let private_tape = generate_private_tape_trace(&record.executed);
        let public_tape = generate_public_tape_trace(&record.executed);
        let call_tape_rows = generate_call_tape_trace(&record.executed);
        let event_tape_rows = generate_event_tape_trace(&record.executed);
        let events_commitment_tape_rows = generate_events_commitment_tape_trace(&record.executed);
        let cast_list_commitment_tape_rows =
            generate_cast_list_commitment_tape_trace(&record.executed);
        let poseidon2_sponge_trace = generate_poseidon2_sponge_trace(&record.executed);
        let poseidon2_output_bytes = generate_poseidon2_output_bytes_trace(&poseidon2_sponge_trace);
        let trace_poly_values = trace_rows_to_poly_values(generate_memory_trace(
            &record.executed,
            &memory_init,
            &memory_zeroinit_rows,
            &halfword_memory,
            &fullword_memory,
            &private_tape,
            &public_tape,
            &call_tape_rows,
            &event_tape_rows,
            &events_commitment_tape_rows,
            &cast_list_commitment_tape_rows,
            &poseidon2_sponge_trace,
            &poseidon2_output_bytes,
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
    fn prove_and_verify(_program: &Program, record: &ExecutionRecord<F>) -> Result<()> {
        type S = HalfWordMemoryStark<F, D>;
        let config = fast_test_config();

        let stark = S::default();
        let trace_poly_values =
            trace_rows_to_poly_values(generate_halfword_memory_trace(&record.executed));
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
    fn prove_and_verify(_program: &Program, record: &ExecutionRecord<F>) -> Result<()> {
        type S = FullWordMemoryStark<F, D>;
        let config = fast_test_config();

        let stark = S::default();
        let trace_poly_values =
            trace_rows_to_poly_values(generate_fullword_memory_trace(&record.executed));
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

impl ProveAndVerify for StorageDeviceStark<F, D> {
    fn prove_and_verify(_program: &Program, record: &ExecutionRecord<F>) -> Result<()> {
        type S = StorageDeviceStark<F, D>;
        let config = fast_test_config();

        let stark = S::default();
        let trace_poly_values =
            trace_rows_to_poly_values(generate_private_tape_trace(&record.executed));
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
    fn prove_and_verify(_program: &Program, record: &ExecutionRecord<F>) -> Result<()> {
        type S = BitshiftStark<F, D>;
        let config = fast_test_config();

        let stark = S::default();
        let cpu_rows = generate_cpu_trace::<F>(record);
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
    fn prove_and_verify(_program: &Program, record: &ExecutionRecord<F>) -> Result<()> {
        type S = RegisterInitStark<F, D>;
        let config = fast_test_config();

        let stark = S::default();
        let trace = generate_register_init_trace::<F>(record);
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
    fn prove_and_verify(_program: &Program, record: &ExecutionRecord<F>) -> Result<()> {
        type S = RegisterStark<F, D>;
        let config = fast_test_config();

        let stark = S::default();
        let cpu_trace = generate_cpu_trace(record);
        let add_trace = ops::add::generate(record);
        let blt_trace = ops::blt_taken::generate(record);
        let private_tape = generate_private_tape_trace(&record.executed);
        let public_tape = generate_public_tape_trace(&record.executed);
        let call_tape = generate_call_tape_trace(&record.executed);
        let event_tape = generate_event_tape_trace(&record.executed);
        let events_commitment_tape_rows = generate_events_commitment_tape_trace(&record.executed);
        let cast_list_commitment_tape_rows =
            generate_cast_list_commitment_tape_trace(&record.executed);
        let poseidon2_sponge_rows = generate_poseidon2_sponge_trace(&record.executed);

        let register_init = generate_register_init_trace(record);
        let (_, _, trace) = generate_register_trace(
            &cpu_trace,
            &add_trace,
            &blt_trace,
            &poseidon2_sponge_rows,
            &private_tape,
            &public_tape,
            &call_tape,
            &event_tape,
            &events_commitment_tape_rows,
            &cast_list_commitment_tape_rows,
            &register_init,
        );
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

impl ProveAndVerify for TapeCommitmentsStark<F, D> {
    fn prove_and_verify(_program: &Program, record: &ExecutionRecord<F>) -> Result<()> {
        type S = TapeCommitmentsStark<F, D>;
        let stark = S::default();
        let config = fast_test_config();
        let trace = generate_tape_commitments_trace(record);
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
    /// Prove and verify a [Program] with Mozak RISC-V VM
    ///
    /// Note that this variant is a lot slower than the others, because
    /// this proves and verifies ALL starks and lookups within the Mozak
    /// ZKVM. This should be preferred if the test is concerned with the
    /// consistency of the final [`MozakStark`].
    ///
    /// ## Parameters
    /// `program`: A serialized ELF Program
    /// `record`: Non-constrained execution trace generated by the runner
    fn prove_and_verify(program: &Program, record: &ExecutionRecord<F>) -> Result<()> {
        let config = fast_test_config();
        prove_and_verify_mozak_stark(program, record, &config)
    }
}

pub fn prove_and_verify_mozak_stark(
    program: &Program,
    record: &ExecutionRecord<F>,
    config: &StarkConfig,
) -> Result<()> {
    let stark = MozakStark::default();
    let public_inputs = PublicInputs {
        entry_point: from_u32(program.entry_point),
    };

    let all_proof = prove::<F, C, D>(
        program,
        record,
        &stark,
        config,
        public_inputs,
        &mut TimingTree::default(),
    )?;
    verify_proof(&stark, all_proof, config)
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

pub struct Poseidon2Test {
    pub data: String,
    pub input_start_addr: u32,
    pub output_start_addr: u32,
}

#[must_use]
pub fn create_poseidon2_test(
    test_data: &[Poseidon2Test],
) -> (Program, ExecutionRecord<GoldilocksField>) {
    let mut instructions = vec![];
    let mut memory: Vec<(u32, u8)> = vec![];

    for test_datum in test_data {
        let mut data_bytes = test_datum.data.as_bytes().to_vec();
        // VM expects input len to be multiple of RATE bits
        data_bytes.resize(data_bytes.len().next_multiple_of(8), 0_u8);
        let data_len = data_bytes.len();
        let input_memory: Vec<(u32, u8)> =
            izip!((test_datum.input_start_addr..), data_bytes).collect();
        memory.extend(input_memory);
        instructions.extend(&[
            Instruction {
                op: Op::ADD,
                args: Args {
                    rd: REG_A0,
                    imm: ecall::POSEIDON2,
                    ..Args::default()
                },
            },
            Instruction {
                op: Op::ADD,
                args: Args {
                    rd: REG_A1,
                    imm: test_datum.input_start_addr,
                    ..Args::default()
                },
            },
            Instruction {
                op: Op::ADD,
                args: Args {
                    rd: REG_A2,
                    imm: u32::try_from(data_len).expect("don't use very long data"),
                    ..Args::default()
                },
            },
            Instruction {
                op: Op::ADD,
                args: Args {
                    rd: REG_A3,
                    imm: test_datum.output_start_addr,
                    ..Args::default()
                },
            },
            ECALL,
        ]);
    }

    code::execute(instructions, memory.as_slice(), &[])
}

pub fn hash_str(v: &str) -> HashOut<F> {
    let v: Vec<_> = v.bytes().map(F::from_canonical_u8).collect();
    Poseidon2Hash::hash_no_pad(&v)
}

pub fn hash_branch<F: RichField>(left: &HashOut<F>, right: &HashOut<F>) -> HashOut<F> {
    let [l0, l1, l2, l3] = left.elements;
    let [r0, r1, r2, r3] = right.elements;
    Poseidon2Hash::hash_no_pad(&[l0, l1, l2, l3, r0, r1, r2, r3])
}

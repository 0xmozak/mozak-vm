use anyhow::Result;
use mozak_circuits::stark::batch_prover::batch_prove;
use mozak_circuits::stark::batch_verifier::batch_verify_proof;
use mozak_circuits::stark::mozak_stark::{MozakStark, PublicInputs, PUBLIC_TABLE_KINDS};
use mozak_circuits::stark::proof::{AllProof, BatchProof};
use mozak_circuits::stark::prover::prove;
use mozak_circuits::stark::recursive_verifier::{
    recursive_batch_stark_circuit, recursive_mozak_stark_circuit, MozakBatchStarkVerifierCircuit,
    MozakStarkVerifierCircuit,
};
use mozak_circuits::stark::verifier::verify_proof;
use mozak_circuits::test_utils::{
    prove_and_verify_batch_mozak_stark, prove_and_verify_mozak_stark, C, D, F,
};
use mozak_examples::VECTOR_ALLOC_ELF;
use mozak_runner::elf::Program;
use mozak_runner::state::{RawTapes, State};
use mozak_runner::vm::{step, ExecutionRecord};
use plonky2::field::types::Field;
use plonky2::plonk::circuit_data::CircuitConfig;
use plonky2::util::timing::TimingTree;
use starky::config::StarkConfig;

use super::benches::Bench;

pub fn vector_alloc_execute(result: Result<(Program, ExecutionRecord<F>)>) -> Result<()> {
    let (program, record) = result?;
    prove_and_verify_mozak_stark(&program, &record, &StarkConfig::standard_fast_config())
}

pub fn vector_alloc_prepare(n: u32) -> Result<(Program, ExecutionRecord<F>)> {
    let program = Program::vanilla_load_elf(VECTOR_ALLOC_ELF)?;
    let raw_tapes = RawTapes {
        public_tape: n.to_le_bytes().to_vec(),
        ..Default::default()
    };
    let state = State::new(program.clone(), raw_tapes);
    let record = step(&program, state)?;
    Ok((program, record))
}

/// Returns the stark proof for `VECTOR_ALLOC_ELF`, and its corresponding
/// `RecursiveVerifierCircuit`.
pub fn vector_alloc_recursive_prepare(
    n: u32,
) -> Result<(MozakStarkVerifierCircuit<F, C, D>, AllProof<F, C, D>)> {
    let mozak_stark = MozakStark::default();
    let stark_config = StarkConfig::standard_fast_config();
    let (program, record) = vector_alloc_prepare(n)?;
    let public_inputs = PublicInputs {
        entry_point: F::from_canonical_u32(program.entry_point),
    };
    let mozak_proof = prove::<F, C, D>(
        &program,
        &record,
        &mozak_stark,
        &stark_config,
        public_inputs,
        &mut TimingTree::default(),
    )?;
    verify_proof(&mozak_stark, mozak_proof.clone(), &stark_config)?;
    let circuit_config = CircuitConfig::standard_recursion_config();
    let mozak_stark_circuit = recursive_mozak_stark_circuit::<F, C, D>(
        &mozak_stark,
        &mozak_proof.degree_bits(&stark_config),
        &circuit_config,
        &stark_config,
    );
    Ok((mozak_stark_circuit, mozak_proof))
}

/// Recursively verifies the stark proof for `VECTOR_ALLOC_ELF`, with
/// its `MozakStarkVerifierCircuit`
pub fn vector_alloc_recursive_execute(
    circuit_with_proof: Result<(MozakStarkVerifierCircuit<F, C, D>, AllProof<F, C, D>)>,
) -> Result<()> {
    let (mozak_stark_circuit, mozak_proof) = circuit_with_proof?;
    let recursive_proof = mozak_stark_circuit.prove(&mozak_proof)?;
    mozak_stark_circuit.circuit.verify(recursive_proof)
}

pub fn batch_starks_vector_alloc_execute(result: Result<(Program, ExecutionRecord<F>)>) -> Result<()> {
    let (program, record) = result?;
    prove_and_verify_batch_mozak_stark(&program, &record, &StarkConfig::standard_fast_config())
}

pub fn batch_starks_vector_alloc_recursive_prepare(
    n: u32,
) -> Result<(MozakBatchStarkVerifierCircuit<F, C, D>, BatchProof<F, C, D>)> {
    let mozak_stark = MozakStark::default();
    let stark_config = StarkConfig::standard_fast_config();
    let (program, record) = vector_alloc_prepare(n)?;
    let public_inputs = PublicInputs {
        entry_point: F::from_canonical_u32(program.entry_point),
    };
    let (mozak_proof, degree_bits) = batch_prove::<F, C, D>(
        &program,
        &record,
        &mozak_stark,
        &PUBLIC_TABLE_KINDS,
        &stark_config,
        public_inputs,
        &mut TimingTree::default(),
    )?;
    batch_verify_proof(
        &mozak_stark,
        &PUBLIC_TABLE_KINDS,
        mozak_proof.clone(),
        &stark_config,
        &degree_bits,
    )?;
    let circuit_config = CircuitConfig::standard_recursion_config();
    let mozak_stark_circuit = recursive_batch_stark_circuit::<F, C, D>(
        &mozak_stark,
        &degree_bits,
        &PUBLIC_TABLE_KINDS,
        &circuit_config,
        &stark_config,
    );
    Ok((mozak_stark_circuit, mozak_proof))
}

pub fn batch_starks_vector_alloc_recursive_execute(
    circuit_with_proof: Result<(MozakBatchStarkVerifierCircuit<F, C, D>, BatchProof<F, C, D>)>,
) -> Result<()> {
    let (mozak_stark_circuit, mozak_proof) = circuit_with_proof?;
    let recursive_proof = mozak_stark_circuit.prove(&mozak_proof)?;
    mozak_stark_circuit.circuit.verify(recursive_proof)
}

pub(crate) struct VectorAllocBench;

impl Bench for VectorAllocBench {
    type Args = u32;
    type Prepared = Result<(Program, ExecutionRecord<F>)>;

    fn prepare(&self, args: &Self::Args) -> Self::Prepared { vector_alloc_prepare(*args) }

    fn execute(&self, prepared: Self::Prepared) -> Result<()> { vector_alloc_execute(prepared) }
}

pub(crate) struct VectorAllocBenchRecursive;

impl Bench for VectorAllocBenchRecursive {
    type Args = u32;
    type Prepared = Result<(MozakStarkVerifierCircuit<F, C, D>, AllProof<F, C, D>)>;

    fn prepare(&self, args: &Self::Args) -> Self::Prepared { vector_alloc_recursive_prepare(*args) }

    fn execute(&self, prepared: Self::Prepared) -> Result<()> { vector_alloc_recursive_execute(prepared) }
}

pub(crate) struct BatchStarksVectorAllocBench;

impl Bench for BatchStarksVectorAllocBench {
    type Args = u32;
    type Prepared = Result<(Program, ExecutionRecord<F>)>;

    fn prepare(&self, args: &Self::Args) -> Self::Prepared { vector_alloc_prepare(*args) }

    fn execute(&self, prepared: Self::Prepared) -> Result<()> {
        batch_starks_vector_alloc_execute(prepared)
    }
}

pub(crate) struct BatchStarksVectorAllocBenchRecursive;

impl Bench for BatchStarksVectorAllocBenchRecursive {
    type Args = u32;
    type Prepared = Result<(MozakBatchStarkVerifierCircuit<F, C, D>, BatchProof<F, C, D>)>;

    fn prepare(&self, args: &Self::Args) -> Self::Prepared {
        batch_starks_vector_alloc_recursive_prepare(*args)
    }

    fn execute(&self, prepared: Self::Prepared) -> Result<()> {
        batch_starks_vector_alloc_recursive_execute(prepared)
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;

    use super::{
        batch_starks_vector_alloc_execute, batch_starks_vector_alloc_recursive_execute,
        batch_starks_vector_alloc_recursive_prepare, vector_alloc_execute, vector_alloc_prepare, vector_alloc_recursive_execute,
        vector_alloc_recursive_prepare,
    };

    #[test]
    fn test_vector_alloc_bench() -> Result<()> {
        let n = 10;
        vector_alloc_execute(vector_alloc_prepare(n))
    }

    #[test]
    fn test_recursive_vector_alloc_bench() -> Result<()> {
        let n = 10;
        vector_alloc_recursive_execute(vector_alloc_recursive_prepare(n))
    }

    #[test]
    fn test_batch_starks_vector_alloc_bench() -> Result<()> {
        let n = 10;
        batch_starks_vector_alloc_execute(vector_alloc_prepare(n))
    }

    #[test]
    fn test_batch_stark_recursive_vector_alloc_bench() -> Result<()> {
        let n = 10;
        batch_starks_vector_alloc_recursive_execute(batch_starks_vector_alloc_recursive_prepare(n))
    }
}

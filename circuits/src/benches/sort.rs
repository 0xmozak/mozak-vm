use anyhow::Result;
use mozak_examples::MOZAK_SORT_ELF;
use mozak_runner::elf::Program;
use mozak_runner::state::{RawTapes, State};
use mozak_runner::vm::{step, ExecutionRecord};
use plonky2::field::types::Field;
use plonky2::plonk::circuit_data::CircuitConfig;
use plonky2::util::timing::TimingTree;
use starky::config::StarkConfig;

use super::Bench;
use crate::stark::mozak_stark::{MozakStark, PublicInputs};
use crate::stark::proof::AllProof;
use crate::stark::prover::prove;
use crate::stark::recursive_verifier::{recursive_mozak_stark_circuit, MozakStarkVerifierCircuit};
use crate::stark::verifier::verify_proof;
use crate::test_utils::{prove_and_verify_mozak_stark, C, D, F};

pub(crate) struct SortBench;

impl Bench for SortBench {
    type Args = u32;
    type Prepared = Result<(Program, ExecutionRecord<F>)>;

    fn prepare(&self, &n: &u32) -> Self::Prepared {
        let program = Program::vanilla_load_elf(MOZAK_SORT_ELF)?;
        let raw_tapes = RawTapes {
            public_tape: n.to_le_bytes().to_vec(),
            ..Default::default()
        };
        let state = State::new(program.clone(), raw_tapes);
        let record = step(&program, state)?;
        Ok((program, record))
    }

    fn execute(&self, result: Result<(Program, ExecutionRecord<F>)>) -> Result<()> {
        let (program, record) = result?;
        prove_and_verify_mozak_stark(&program, &record, &StarkConfig::standard_fast_config())
    }
}

pub(crate) struct SortBenchRecursive;

impl Bench for SortBenchRecursive {
    type Args = u32;
    type Prepared = Result<(MozakStarkVerifierCircuit<F, C, D>, AllProof<F, C, D>)>;

    /// Returns the stark proof for `MOZAK_SORT_ELF`, and its corresponding
    /// `RecursiveVerifierCircuit`.
    fn prepare(&self, &n: &u32) -> Self::Prepared {
        let mozak_stark = MozakStark::default();
        let stark_config = StarkConfig::standard_fast_config();
        let (program, record) = SortBench {}.prepare(&n)?;
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

    /// Recursively verifies the stark proof for `MOZAK_SORT_ELF`, with
    /// its `MozakStarkVerifierCircuit`
    fn execute(
        &self,
        circuit_with_proof: Result<(MozakStarkVerifierCircuit<F, C, D>, AllProof<F, C, D>)>,
    ) -> Result<()> {
        let (mozak_stark_circuit, mozak_proof) = circuit_with_proof?;
        let recursive_proof = mozak_stark_circuit.prove(&mozak_proof)?;
        mozak_stark_circuit.circuit.verify(recursive_proof)
    }
}
#[cfg(test)]
mod tests {
    use anyhow::Result;

    use super::{SortBench, SortBenchRecursive};
    use crate::benches::Bench;

    #[test]
    fn test_sort_bench() -> Result<()> { SortBench {}.execute(SortBench {}.prepare(&10)) }
    #[test]
    fn test_recursive_sort_bench() -> Result<()> {
        SortBenchRecursive {}.execute(SortBenchRecursive {}.prepare(&10))
    }
}

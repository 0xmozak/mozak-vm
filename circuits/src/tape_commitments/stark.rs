use core::fmt::Debug;

use expr::Expr;
use mozak_circuits_derive::StarkNameDisplay;

use super::columns::TapeCommitments;
use crate::columns_view::{HasNamedColumns, NumberOfColumns};
use crate::expr::{ConstraintBuilder, GenerateConstraints, StarkFrom, Vars};
use crate::unstark::NoColumns;

impl GenerateConstraints<COLUMNS, PUBLIC_INPUTS>
    for TapeCommitmentsConstraints
{
    type PublicInputs<E: Debug> = NoColumns<E>;
    type View<E: Debug> = TapeCommitments<E>;

    fn generate_constraints<'a, T: Copy + Debug>(
        &self,
        vars: &Vars<'a, Self, T, COLUMNS, PUBLIC_INPUTS>,
    ) -> ConstraintBuilder<Expr<'a, T>> {
        let lv: &TapeCommitments<Expr<'a, T>> = &vars.local_values;
        let mut constraint = ConstraintBuilder::default();
        constraint.always(lv.is_event_commitment_tape_row.is_binary());
        constraint.always(lv.is_castlist_commitment_tape_row.is_binary());
        constraint.always(
            (lv.is_castlist_commitment_tape_row + lv.is_event_commitment_tape_row).is_binary(),
        );
        constraint
            .always(lv.event_commitment_tape_multiplicity * (1 - lv.is_event_commitment_tape_row));
        constraint.always(
            lv.castlist_commitment_tape_multiplicity * (1 - lv.is_castlist_commitment_tape_row),
        );
        constraint
    }
}

#[derive(Copy, Clone, Default, StarkNameDisplay)]
#[allow(clippy::module_name_repetitions)]
pub struct TapeCommitmentsConstraints {}

pub type TapeCommitmentsStark<F, const D: usize> = StarkFrom<F, TapeCommitmentsConstraints, {D}, {COLUMNS}, {PUBLIC_INPUTS}>;

impl<F, const D: usize> HasNamedColumns for TapeCommitmentsStark<F, D> {
    type Columns = TapeCommitments<F>;
}

const COLUMNS: usize = TapeCommitments::<()>::NUMBER_OF_COLUMNS;
const PUBLIC_INPUTS: usize = 0;

#[cfg(test)]
mod tests {
    use itertools::chain;
    use mozak_runner::code;
    use mozak_runner::decode::ECALL;
    use mozak_runner::instruction::{Args, Instruction, Op};
    use mozak_runner::state::RawTapes;
    use mozak_sdk::core::constants::DIGEST_BYTES;
    use mozak_sdk::core::ecall::{self};
    use mozak_sdk::core::reg_abi::{REG_A0, REG_A1, REG_A2};
    use plonky2::field::types::Field;
    use plonky2::plonk::circuit_data::CircuitConfig;
    use plonky2::plonk::config::{GenericConfig, Poseidon2GoldilocksConfig};
    use plonky2::util::timing::TimingTree;
    use rand::Rng;
    use starky::config::StarkConfig;
    use starky::stark_testing::test_stark_circuit_constraints;

    use super::TapeCommitmentsStark;
    use crate::stark::mozak_stark::{MozakStark, PublicInputs};
    use crate::stark::prover::prove;
    use crate::stark::recursive_verifier::{
        recursive_mozak_stark_circuit, VMRecursiveProofPublicInputs, VM_PUBLIC_INPUT_SIZE,
    };
    use crate::stark::verifier::verify_proof;
    use crate::test_utils::ProveAndVerify;
    use crate::utils::from_u32;

    const D: usize = 2;
    type C = Poseidon2GoldilocksConfig;
    type F = <C as GenericConfig<D>>::F;
    type S = super::TapeCommitmentsStark<F, D>;

    const CAST_LIST_COMMITMENT_ADDRESS: u32 = 0x100;
    const EVENTS_COMMITMENT_ADDRESS: u32 = 0x200;

    fn read_tape_commitments_code() -> Vec<Instruction> {
        fn read_ecall_code(ecall: u32, address: u32, num_bytes_read: usize) -> Vec<Instruction> {
            vec![
                Instruction {
                    op: Op::ADD,
                    args: Args {
                        rd: REG_A0,
                        imm: ecall,
                        ..Default::default()
                    },
                },
                Instruction {
                    op: Op::ADD,
                    args: Args {
                        rd: REG_A1,
                        imm: address,
                        ..Default::default()
                    },
                },
                Instruction {
                    op: Op::ADD,
                    args: Args {
                        rd: REG_A2,
                        imm: u32::try_from(num_bytes_read).expect("casting to u32 should not fail"),
                        ..Default::default()
                    },
                },
                ECALL,
            ]
        }
        let code_ecall_cast_list_commitment_tape = read_ecall_code(
            ecall::CAST_LIST_COMMITMENT_TAPE,
            CAST_LIST_COMMITMENT_ADDRESS,
            DIGEST_BYTES,
        );
        let code_ecall_events_commitment_tape = read_ecall_code(
            ecall::EVENTS_COMMITMENT_TAPE,
            EVENTS_COMMITMENT_ADDRESS,
            DIGEST_BYTES,
        );
        chain!(
            code_ecall_cast_list_commitment_tape,
            code_ecall_events_commitment_tape
        )
        .collect()
    }

    #[test]
    fn test_tape_commitment_stark() -> Result<(), anyhow::Error> {
        let mut rng = rand::thread_rng();
        // generate tapes with random bytes
        let cast_list_commitment_tape: [u8; DIGEST_BYTES] = rng.gen();
        let events_commitment_tape: [u8; DIGEST_BYTES] = rng.gen();
        let code = read_tape_commitments_code();
        let (program, record) = code::execute_code_with_ro_memory(code, &[], &[], &[], RawTapes {
            events_commitment_tape,
            cast_list_commitment_tape,
            ..Default::default()
        });
        TapeCommitmentsStark::prove_and_verify(&program, &record)
    }
    #[test]
    fn test_tape_commitment_mozak_stark() -> Result<(), anyhow::Error> {
        let mut rng = rand::thread_rng();
        // generate tapes with random bytes
        let cast_list_commitment_tape: [u8; DIGEST_BYTES] = rng.gen();
        let events_commitment_tape: [u8; DIGEST_BYTES] = rng.gen();
        let code = read_tape_commitments_code();
        let (program, record) = code::execute_code_with_ro_memory(code, &[], &[], &[], RawTapes {
            events_commitment_tape,
            cast_list_commitment_tape,
            ..Default::default()
        });
        MozakStark::prove_and_verify(&program, &record)
    }

    #[test]
    fn test_tape_commitment_recursive_prover() -> Result<(), anyhow::Error> {
        let mut rng = rand::thread_rng();
        // generate tapes with random bytes
        let cast_list_commitment_tape: [u8; DIGEST_BYTES] = rng.gen();
        let events_commitment_tape: [u8; DIGEST_BYTES] = rng.gen();
        let code = read_tape_commitments_code();
        let (program, record) = code::execute_code_with_ro_memory(code, &[], &[], &[], RawTapes {
            events_commitment_tape,
            cast_list_commitment_tape,
            ..Default::default()
        });
        let stark = MozakStark::<F, D>::default();
        let config = StarkConfig::standard_fast_config();
        let public_inputs = PublicInputs {
            entry_point: from_u32(program.entry_point),
        };
        let mozak_proof = prove::<F, C, D>(
            &program,
            &record,
            &stark,
            &config,
            public_inputs,
            &mut TimingTree::default(),
        )?;
        verify_proof(&stark, mozak_proof.clone(), &config)?;

        let circuit_config = CircuitConfig::standard_recursion_config();
        let mozak_stark_circuit = recursive_mozak_stark_circuit::<F, C, D>(
            &stark,
            &mozak_proof.degree_bits(&config),
            &circuit_config,
            &config,
        );

        let recursive_proof = mozak_stark_circuit.prove(&mozak_proof)?;
        let public_input_slice: [F; VM_PUBLIC_INPUT_SIZE] =
            recursive_proof.public_inputs.as_slice().try_into().unwrap();
        let recursive_proof_public_inputs: &VMRecursiveProofPublicInputs<F> =
            &public_input_slice.into();

        // assert that the commitment tapes match those in pubilc inputs
        assert_eq!(
            recursive_proof_public_inputs.event_commitment_tape,
            events_commitment_tape.map(F::from_canonical_u8),
            "Mismatch in events commitment tape in public inputs"
        );
        assert_eq!(
            recursive_proof_public_inputs.castlist_commitment_tape,
            cast_list_commitment_tape.map(F::from_canonical_u8),
            "Mismatch in cast list commitment tape in public inputs"
        );
        mozak_stark_circuit.circuit.verify(recursive_proof)
    }

    #[test]
    fn test_circuit() -> anyhow::Result<()> {
        let stark = S::default();
        test_stark_circuit_constraints::<F, C, S, D>(stark)?;

        Ok(())
    }
}

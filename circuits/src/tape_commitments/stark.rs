use core::fmt::Debug;
use std::marker::PhantomData;

use expr::{Expr, ExprBuilder, StarkFrameTyped};
use mozak_circuits_derive::StarkNameDisplay;
use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};
use starky::evaluation_frame::StarkFrame;
use starky::stark::Stark;

use super::columns::TapeCommitments;
use crate::columns_view::{HasNamedColumns, NumberOfColumns};
use crate::expr::{build_ext, build_packed, ConstraintBuilder, GenerateConstraints};
use crate::unstark::NoColumns;

impl<'a, F, T: Copy + Debug + 'a, const D: usize>
    GenerateConstraints<'a, T>
    for TapeCommitmentsStark<F, { D }>
{
    type View<E: Debug + 'a> = TapeCommitments<E>;
    type PublicInputs<E: Debug + 'a> = NoColumns<E>;

    fn generate_constraints(
        vars: &StarkFrameTyped<TapeCommitments<Expr<'a, T>>, NoColumns<Expr<'a, T>>>,
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
pub struct TapeCommitmentsStark<F, const D: usize> {
    pub _f: PhantomData<F>,
}

impl<F, const D: usize> HasNamedColumns for TapeCommitmentsStark<F, D> {
    type Columns = TapeCommitments<F>;
}

const COLUMNS: usize = TapeCommitments::<()>::NUMBER_OF_COLUMNS;
const PUBLIC_INPUTS: usize = 0;

impl<F: RichField + Extendable<D>, const D: usize> Stark<F, D> for TapeCommitmentsStark<F, D> {
    type EvaluationFrame<FE, P, const D2: usize> = StarkFrame<P, P::Scalar, COLUMNS, PUBLIC_INPUTS>

        where
            FE: FieldExtension<D2, BaseField = F>,
            P: PackedField<Scalar = FE>;
    type EvaluationFrameTarget =
        StarkFrame<ExtensionTarget<D>, ExtensionTarget<D>, COLUMNS, PUBLIC_INPUTS>;

    fn eval_packed_generic<FE, P, const D2: usize>(
        &self,
        vars: &Self::EvaluationFrame<FE, P, D2>,
        consumer: &mut ConstraintConsumer<P>,
    ) where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>, {
        let eb = ExprBuilder::default();
        let constraints = Self::generate_constraints(&eb.to_typed_starkframe(vars));
        build_packed(constraints, consumer);
    }

    fn constraint_degree(&self) -> usize { 3 }

    fn eval_ext_circuit(
        &self,
        builder: &mut CircuitBuilder<F, D>,
        vars: &Self::EvaluationFrameTarget,
        consumer: &mut RecursiveConstraintConsumer<F, D>,
    ) {
        let eb = ExprBuilder::default();
        let constraints = Self::generate_constraints(&eb.to_typed_starkframe(vars));
        build_ext(constraints, builder, consumer);
    }
}

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

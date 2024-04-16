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
use crate::expr::{build_ext, build_packed, ConstraintBuilder};
fn generate_constraints<'a, T: Copy, U, const N2: usize>(
    vars: &StarkFrameTyped<TapeCommitments<Expr<'a, T>>, [U; N2]>,
) -> ConstraintBuilder<Expr<'a, T>> {
    let lv: &TapeCommitments<Expr<'a, T>> = &vars.local_values;
    let mut constraint = ConstraintBuilder::default();
    constraint.always(lv.is_event_commitment_tape_row.is_binary());
    constraint.always(lv.is_castlist_commitment_tape_row.is_binary());
    constraint
        .always((lv.is_castlist_commitment_tape_row + lv.is_event_commitment_tape_row).is_binary());
    constraint
        .always(lv.event_commitment_tape_multiplicity * (1 - lv.is_event_commitment_tape_row));
    constraint.always(
        lv.castlist_commitment_tape_multiplicity * (1 - lv.is_castlist_commitment_tape_row),
    );
    constraint
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
        let constraints = generate_constraints(&eb.to_typed_starkframe(vars));
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
        let constraints = generate_constraints(&eb.to_typed_starkframe(vars));
        build_ext(constraints, builder, consumer);
    }
}

#[cfg(test)]
mod tests {
    use itertools::{chain, Itertools};
    use mozak_runner::code;
    use mozak_runner::decode::ECALL;
    use mozak_runner::elf::RuntimeArguments;
    use mozak_runner::instruction::{Args, Instruction, Op};
    use mozak_sdk::core::ecall::{self, COMMITMENT_SIZE};
    use mozak_sdk::core::reg_abi::{REG_A0, REG_A1, REG_A2};
    use plonky2::plonk::circuit_data::CircuitConfig;
    use plonky2::plonk::config::{GenericConfig, Poseidon2GoldilocksConfig};
    use plonky2::util::timing::TimingTree;
    use starky::config::StarkConfig;
    use starky::stark_testing::test_stark_circuit_constraints;

    use super::TapeCommitmentsStark;
    use crate::stark::mozak_stark::{MozakStark, PublicInputs};
    use crate::stark::prover::prove;
    use crate::stark::recursive_verifier::recursive_mozak_stark_circuit;
    use crate::stark::verifier::verify_proof;
    use crate::tape_commitments::columns::{
        get_castlist_commitment_tape_from_proof, get_event_commitment_tape_from_proof,
    };
    use crate::test_utils::ProveAndVerify;
    use crate::utils::from_u32;

    const D: usize = 2;
    type C = Poseidon2GoldilocksConfig;
    type F = <C as GenericConfig<D>>::F;
    type S = super::TapeCommitmentsStark<F, D>;
    use plonky2::field::types::Field;

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_tape_commitment_stark() -> Result<(), anyhow::Error> {
        let cast_list_commitment_address = 0x100;
        let events_commitment_address = 0x200;
        let code_ecall_cast_list_commitment_tape = [
            Instruction {
                op: Op::ADD,
                args: Args {
                    rd: REG_A0,
                    rs1: 1,
                    rs2: 2,
                    imm: ecall::CAST_LIST_COMMITMENT_TAPE,
                },
            },
            Instruction {
                op: Op::ADD,
                args: Args {
                    rd: REG_A1,
                    rs1: 1,
                    rs2: 2,
                    imm: cast_list_commitment_address,
                },
            },
            Instruction {
                op: Op::ADD,
                args: Args {
                    rd: REG_A2,
                    rs1: 1,
                    rs2: 2,
                    imm: u32::try_from(COMMITMENT_SIZE).expect("casting 32 to u32 should not fail"),
                },
            },
            ECALL,
        ];
        let code_ecall_events_commitment_tape = [
            Instruction {
                op: Op::ADD,
                args: Args {
                    rd: REG_A0,
                    rs1: 1,
                    rs2: 2,
                    imm: ecall::EVENTS_COMMITMENT_TAPE,
                },
            },
            Instruction {
                op: Op::ADD,
                args: Args {
                    rd: REG_A1,
                    rs1: 1,
                    rs2: 2,
                    imm: events_commitment_address,
                },
            },
            Instruction {
                op: Op::ADD,
                args: Args {
                    rd: REG_A2,
                    rs1: 1,
                    rs2: 2,
                    imm: u32::try_from(COMMITMENT_SIZE).expect("casting 32 to u32 should not fail"),
                },
            },
            ECALL,
        ];
        let cast_list_commitment_tape = (0..32).collect_vec();
        let events_commitment_tape = (32..64).collect_vec();
        let (program, record) = code::execute_code_with_ro_memory(
            chain!(
                code_ecall_cast_list_commitment_tape.into_iter(),
                code_ecall_events_commitment_tape.into_iter(),
            ),
            &[],
            &[],
            &[(1, 0), (2, 0)],
            RuntimeArguments {
                cast_list_commitment_tape: cast_list_commitment_tape.clone(),
                events_commitment_tape: events_commitment_tape.clone(),
                ..Default::default()
            },
        );
        TapeCommitmentsStark::prove_and_verify(&program, &record)?;

        let stark = MozakStark::<F, D>::default();
        let mut config = StarkConfig::standard_fast_config();
        config.fri_config.cap_height = 1;
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
        assert_eq!(
            get_event_commitment_tape_from_proof(&recursive_proof),
            events_commitment_tape
                .into_iter()
                .map(F::from_canonical_u8)
                .collect_vec(),
            "Could not find expected_event_commitment_tape in recursive proof's public inputs"
        );
        assert_eq!(
            get_castlist_commitment_tape_from_proof(&recursive_proof),
            cast_list_commitment_tape
                .into_iter()
                .map(F::from_canonical_u8)
                .collect_vec(),
            "Could not find expected_castlist_commitment_tape in recursive proof's public inputs"
        );
        mozak_stark_circuit.circuit.verify(recursive_proof)?;
        Ok(())
    }

    #[test]
    fn test_circuit() -> anyhow::Result<()> {
        let stark = S::default();
        test_stark_circuit_constraints::<F, C, S, D>(stark)?;

        Ok(())
    }
}

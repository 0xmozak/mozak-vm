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
    constraint.always(
        lv.event_commitment_tape_multiplicity * (1 - lv.event_commitment_tape_multiplicity),
    );
    constraint.always(
        lv.castlist_commitment_tape_multiplicity * (1 - lv.castlist_commitment_tape_multiplicity),
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

    fn constraint_degree(&self) -> usize { 2 }

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
    use mozak_runner::code;
    use mozak_runner::instruction::{Args, Instruction, Op};
    use plonky2::plonk::config::{GenericConfig, Poseidon2GoldilocksConfig};
    use starky::stark_testing::test_stark_circuit_constraints;

    use super::TapeCommitmentsStark;
    use crate::test_utils::ProveAndVerify;

    const D: usize = 2;
    type C = Poseidon2GoldilocksConfig;
    type F = <C as GenericConfig<D>>::F;
    type S = super::TapeCommitmentsStark<F, D>;

    #[test]
    fn test_tape_commitment_stark() -> anyhow::Result<()> {
        let (program, record) = code::execute(
            [Instruction {
                op: Op::ADD,
                args: Args {
                    rd: 5,
                    rs1: 6,
                    rs2: 7,
                    ..Args::default()
                },
            }],
            &[],
            &[(6, 100), (7, 100)],
        );
        TapeCommitmentsStark::prove_and_verify(&program, &record)
    }

    #[test]
    fn test_circuit() -> anyhow::Result<()> {
        let stark = S::default();
        test_stark_circuit_constraints::<F, C, S, D>(stark)?;

        Ok(())
    }
}

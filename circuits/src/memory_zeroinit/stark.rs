use mozak_circuits_derive::StarkNameDisplay;
use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};
use starky::evaluation_frame::{StarkEvaluationFrame, StarkFrame};
use starky::stark::Stark;

use super::columns::MemoryZeroInit;
use crate::columns_view::{stark_impl, HasNamedColumns, NumberOfColumns};
use crate::stark::utils::{is_binary, is_binary_ext_circuit};

#[derive(Clone, Copy, Default, StarkNameDisplay)]
#[allow(clippy::module_name_repetitions)]
pub struct MemoryZeroInitStark<F, const D: usize> {
    pub conjunctive_challenge: F,
}
stark_impl!(MemoryZeroInitStark);

impl<F, const D: usize> HasNamedColumns for MemoryZeroInitStark<F, D> {
    type Columns = MemoryZeroInit<F>;
}

const COLUMNS: usize = MemoryZeroInit::<()>::NUMBER_OF_COLUMNS;
const PUBLIC_INPUTS: usize = 0;

impl<F: RichField + Extendable<D>, const D: usize> Stark<F, D> for MemoryZeroInitStark<F, D> {
    type EvaluationFrame<FE, P, const D2: usize> = StarkFrame<P, P::Scalar, COLUMNS, PUBLIC_INPUTS>

    where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>;
    type EvaluationFrameTarget =
        StarkFrame<ExtensionTarget<D>, ExtensionTarget<D>, COLUMNS, PUBLIC_INPUTS>;

    fn eval_packed_generic<FE, P, const D2: usize>(
        &self,
        vars: &Self::EvaluationFrame<FE, P, D2>,
        yield_constr: &mut ConstraintConsumer<P>,
    ) where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>, {
        let lv: &MemoryZeroInit<P> = vars.get_local_values().into();
        is_binary(yield_constr, lv.filter);
    }

    fn eval_ext_circuit(
        &self,
        builder: &mut CircuitBuilder<F, D>,
        vars: &Self::EvaluationFrameTarget,
        yield_constr: &mut RecursiveConstraintConsumer<F, D>,
    ) {
        let lv: &MemoryZeroInit<ExtensionTarget<D>> = vars.get_local_values().into();
        is_binary_ext_circuit(builder, lv.filter, yield_constr);
    }

    fn constraint_degree(&self) -> usize { 3 }
}

#[cfg(test)]
mod tests {
    use plonky2::plonk::config::{GenericConfig, Poseidon2GoldilocksConfig};
    use starky::stark_testing::test_stark_circuit_constraints;

    use super::*;

    const D: usize = 2;
    type C = Poseidon2GoldilocksConfig;
    type F = <C as GenericConfig<D>>::F;
    type S = MemoryZeroInitStark<F, D>;

    #[test]
    fn test_circuit() -> anyhow::Result<()> {
        let stark = S::default();
        test_stark_circuit_constraints::<F, C, S, D>(stark)?;

        Ok(())
    }
}

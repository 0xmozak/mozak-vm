use std::marker::PhantomData;

use mozak_circuits_derive::StarkNameDisplay;
use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};
use starky::evaluation_frame::{StarkEvaluationFrame, StarkFrame};
use starky::stark::Stark;

use super::columns::RegisterInit;
use crate::columns_view::{HasNamedColumns, NumberOfColumns};
use crate::stark::utils::{is_binary, is_binary_ext_circuit};

#[derive(Clone, Copy, Default, StarkNameDisplay)]
#[allow(clippy::module_name_repetitions)]
pub struct RegisterInitStark<F, const D: usize> {
    pub _f: PhantomData<F>,
    pub standalone_proving: bool,
}

impl<F, const D: usize> HasNamedColumns for RegisterInitStark<F, D> {
    type Columns = RegisterInit<F>;
}

const COLUMNS: usize = RegisterInit::<()>::NUMBER_OF_COLUMNS;
const PUBLIC_INPUTS: usize = 0;

impl<F: RichField + Extendable<D>, const D: usize> Stark<F, D> for RegisterInitStark<F, D> {
    type EvaluationFrame<FE, P, const D2: usize> = StarkFrame<P, P::Scalar, COLUMNS, PUBLIC_INPUTS>

    where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>;
    type EvaluationFrameTarget =
        StarkFrame<ExtensionTarget<D>, ExtensionTarget<D>, COLUMNS, PUBLIC_INPUTS>;

    fn requires_ctls(&self) -> bool { !self.standalone_proving }

    /// Constraints for the [`RegisterInitStark`].
    ///
    /// For sanity check, we can constrain the register address column to be in
    /// a running sum from 0..=31, but since this fixed table is known to
    /// both prover and verifier, we do not need to do so here.
    // TODO(Matthias): add constraints to force registers to start at 0;
    // but make it so we can turn them off for tests.
    fn eval_packed_generic<FE, P, const D2: usize>(
        &self,
        vars: &Self::EvaluationFrame<FE, P, D2>,
        yield_constr: &mut ConstraintConsumer<P>,
    ) where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>, {
        let lv: &RegisterInit<P> = vars.get_local_values().into();
        // Check: `is_looked_up` is a binary filter column.
        is_binary(yield_constr, lv.is_looked_up);
    }

    fn eval_ext_circuit(
        &self,
        builder: &mut CircuitBuilder<F, D>,
        vars: &Self::EvaluationFrameTarget,
        yield_constr: &mut RecursiveConstraintConsumer<F, D>,
    ) {
        let lv: &RegisterInit<_> = vars.get_local_values().into();
        is_binary_ext_circuit(builder, lv.is_looked_up, yield_constr);
    }

    fn constraint_degree(&self) -> usize { 3 }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use mozak_runner::elf::Program;
    use mozak_runner::vm::ExecutionRecord;
    use plonky2::plonk::config::{GenericConfig, Poseidon2GoldilocksConfig};
    use starky::stark_testing::test_stark_low_degree;

    use super::*;
    use crate::test_utils::ProveAndVerify;

    const D: usize = 2;
    type C = Poseidon2GoldilocksConfig;
    type F = <C as GenericConfig<D>>::F;
    type S = RegisterInitStark<F, D>;

    #[test]
    fn test_degree() -> Result<()> {
        let stark = S {
            standalone_proving: true,
            ..S::default()
        };
        test_stark_low_degree(stark)
    }

    #[test]
    fn prove_reg_init() -> Result<()> {
        let program = Program::default();
        let executed = ExecutionRecord::default();
        RegisterInitStark::prove_and_verify(&program, &executed)?;
        Ok(())
    }
}

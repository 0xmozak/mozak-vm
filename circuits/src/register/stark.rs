use std::borrow::Borrow;
use std::marker::PhantomData;

use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::hash::hash_types::RichField;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};
use starky::stark::Stark;
use starky::vars::{StarkEvaluationTargets, StarkEvaluationVars};

use super::columns::Register;
use crate::columns_view::NumberOfColumns;
use crate::cpu::stark::is_binary;

#[derive(Clone, Copy, Default)]
#[allow(clippy::module_name_repetitions)]
pub struct RegisterStark<F, const D: usize> {
    pub _f: PhantomData<F>,
}

impl<F: RichField + Extendable<D>, const D: usize> Stark<F, D> for RegisterStark<F, D> {
    const COLUMNS: usize = Register::<F>::NUMBER_OF_COLUMNS;
    const PUBLIC_INPUTS: usize = 0;

    /// Constraints for the [`RegisterStark`].
    ///
    /// For sanity check, we can constrain the register address column to be in
    /// a running sum from 0..=31, but since this fixed table is known to
    /// both prover and verifier, we do not need to do so here.
    fn eval_packed_generic<FE, P, const D2: usize>(
        &self,
        vars: StarkEvaluationVars<FE, P, { Self::COLUMNS }, { Self::PUBLIC_INPUTS }>,
        yield_constr: &mut ConstraintConsumer<P>,
    ) where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>, {
        let lv: &Register<P> = vars.local_values.borrow();
    }

    #[no_coverage]
    fn eval_ext_circuit(
        &self,
        _builder: &mut CircuitBuilder<F, D>,
        _vars: StarkEvaluationTargets<D, { Self::COLUMNS }, { Self::PUBLIC_INPUTS }>,
        _yield_constr: &mut RecursiveConstraintConsumer<F, D>,
    ) {
        unimplemented!()
    }

    fn constraint_degree(&self) -> usize { 3 }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use mozak_runner::elf::Program;
    use mozak_runner::vm::ExecutionRecord;
    use plonky2::plonk::config::{GenericConfig, PoseidonGoldilocksConfig};
    use starky::stark_testing::test_stark_low_degree;

    use super::*;
    use crate::test_utils::ProveAndVerify;

    const D: usize = 2;
    type C = PoseidonGoldilocksConfig;
    type F = <C as GenericConfig<D>>::F;
    type S = RegisterStark<F, D>;

    #[test]
    fn test_degree() -> Result<()> {
        let stark = S::default();
        test_stark_low_degree(stark)
    }

    // #[test]
    // fn prove_reg_init() -> Result<()> {
    //     let program = Program::default();
    //     let executed = ExecutionRecord::default();
    //     RegisterStark::prove_and_verify(&program, &executed)?;
    //     Ok(())
    // }
}

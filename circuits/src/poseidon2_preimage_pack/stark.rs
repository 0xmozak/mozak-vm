use std::marker::PhantomData;

use mozak_circuits_derive::StarkNameDisplay;
use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};
use starky::evaluation_frame::StarkFrame;
use starky::stark::Stark;

use super::columns::{Poseidon2PreimagePack, NUM_POSEIDON2_PREIMAGE_PACK_COLS};
use crate::columns_view::HasNamedColumns;

#[derive(Copy, Clone, Default, StarkNameDisplay, Debug)]
#[allow(clippy::module_name_repetitions)]
pub struct Poseidon2PreimagePackStark<F, const D: usize> {
    pub _f: PhantomData<F>,
}

impl<F, const D: usize> HasNamedColumns for Poseidon2PreimagePackStark<F, D> {
    type Columns = Poseidon2PreimagePack<F>;
}

const COLUMNS: usize = NUM_POSEIDON2_PREIMAGE_PACK_COLS;
const PUBLIC_INPUTS: usize = 0;

impl<F: RichField + Extendable<D>, const D: usize> Stark<F, D>
    for Poseidon2PreimagePackStark<F, D>
{
    type EvaluationFrame<FE, P, const D2: usize> = StarkFrame<P, P::Scalar, COLUMNS, PUBLIC_INPUTS>
    where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>;
    type EvaluationFrameTarget =
        StarkFrame<ExtensionTarget<D>, ExtensionTarget<D>, COLUMNS, PUBLIC_INPUTS>;

    fn eval_packed_generic<FE, P, const D2: usize>(
        &self,
        _vars: &Self::EvaluationFrame<FE, P, D2>,
        _yield_constr: &mut ConstraintConsumer<P>,
    ) where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>, {
    }

    fn eval_ext_circuit(
        &self,
        _builder: &mut CircuitBuilder<F, D>,
        _vars: &Self::EvaluationFrameTarget,
        _yield_constr: &mut RecursiveConstraintConsumer<F, D>,
    ) {
    }

    fn constraint_degree(&self) -> usize { 3 }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use plonky2::plonk::config::{GenericConfig, Poseidon2GoldilocksConfig};
    use starky::stark_testing::{test_stark_circuit_constraints, test_stark_low_degree};

    use super::Poseidon2PreimagePackStark;

    const D: usize = 2;
    type C = Poseidon2GoldilocksConfig;
    type F = <C as GenericConfig<D>>::F;
    type S = Poseidon2PreimagePackStark<F, D>;

    #[test]
    // TODO(Roman): FIXME
    #[ignore]
    fn poseidon2_stark_degree() -> Result<()> {
        let stark = S::default();
        test_stark_low_degree(stark)
    }
    #[test]
    fn test_circuit() -> anyhow::Result<()> {
        let stark = S::default();
        test_stark_circuit_constraints::<F, C, S, D>(stark)?;

        Ok(())
    }
}

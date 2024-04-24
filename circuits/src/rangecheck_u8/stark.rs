use std::marker::PhantomData;

use mozak_circuits_derive::StarkNameDisplay;
use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};
use starky::evaluation_frame::{StarkEvaluationFrame, StarkFrame};
use starky::stark::Stark;

use super::columns::RangeCheckU8;
use crate::columns_view::{HasNamedColumns, NumberOfColumns};

#[derive(Copy, Clone, Default, StarkNameDisplay)]
#[allow(clippy::module_name_repetitions)]
pub struct RangeCheckU8Stark<F, const D: usize> {
    pub _f: PhantomData<F>,
    pub standalone_proving: bool,
}

impl<F, const D: usize> HasNamedColumns for RangeCheckU8Stark<F, D> {
    type Columns = RangeCheckU8<F>;
}

const COLUMNS: usize = RangeCheckU8::<()>::NUMBER_OF_COLUMNS;
const PUBLIC_INPUTS: usize = 0;

impl<F: RichField + Extendable<D>, const D: usize> Stark<F, D> for RangeCheckU8Stark<F, D> {
    type EvaluationFrame<FE, P, const D2: usize> = StarkFrame<P, P::Scalar, COLUMNS, PUBLIC_INPUTS>

    where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>;
    type EvaluationFrameTarget =
        StarkFrame<ExtensionTarget<D>, ExtensionTarget<D>, COLUMNS, PUBLIC_INPUTS>;

    fn requires_ctls(&self) -> bool { !self.standalone_proving }

    fn eval_packed_generic<FE, P, const D2: usize>(
        &self,
        vars: &Self::EvaluationFrame<FE, P, D2>,
        yield_constr: &mut ConstraintConsumer<P>,
    ) where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>, {
        let lv: &RangeCheckU8<P> = vars.get_local_values().into();
        let nv: &RangeCheckU8<P> = vars.get_next_values().into();
        // Check: the `element`s form a sequence from 0 to 255
        yield_constr.constraint_first_row(lv.value);
        yield_constr.constraint_transition(nv.value - lv.value - FE::ONE);
        yield_constr.constraint_last_row(lv.value - FE::from_canonical_u8(u8::MAX));
    }

    fn eval_ext_circuit(
        &self,
        builder: &mut CircuitBuilder<F, D>,
        vars: &Self::EvaluationFrameTarget,
        yield_constr: &mut RecursiveConstraintConsumer<F, D>,
    ) {
        let lv: &RangeCheckU8<ExtensionTarget<D>> = vars.get_local_values().into();
        let nv: &RangeCheckU8<ExtensionTarget<D>> = vars.get_next_values().into();
        yield_constr.constraint_first_row(builder, lv.value);
        let one = builder.constant_extension(F::Extension::from_canonical_u8(1));
        let nv_sub_lv = builder.sub_extension(nv.value, lv.value);
        let nv_sub_lv_sub_one = builder.sub_extension(nv_sub_lv, one);
        yield_constr.constraint_transition(builder, nv_sub_lv_sub_one);
        let u8max = builder.constant_extension(F::Extension::from_canonical_u8(u8::MAX));
        let lv_sub_u8max = builder.sub_extension(lv.value, u8max);
        yield_constr.constraint_last_row(builder, lv_sub_u8max);
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
    type S = RangeCheckU8Stark<F, D>;

    #[test]
    fn test_circuit() -> anyhow::Result<()> {
        let stark = S {
            standalone_proving: true,
            ..S::default()
        };
        test_stark_circuit_constraints::<F, C, S, D>(stark)?;

        Ok(())
    }
}

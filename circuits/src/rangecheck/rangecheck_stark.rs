use std::marker::PhantomData;

use plonky2::{
    field::{
        extension::{Extendable, FieldExtension},
        packed::PackedField,
        types::Field,
    },
    hash::hash_types::RichField,
    plonk::circuit_builder::CircuitBuilder,
};
use starky::{
    constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer},
    stark::Stark,
    vars::{StarkEvaluationTargets, StarkEvaluationVars},
};

use super::columns::{self, COL_NUM_RC};

#[derive(Copy, Clone, Default)]
pub struct RangeCheckStark<F, const D: usize> {
    pub f: PhantomData<F>,
}

const RANGE_MAX: usize = 1usize << 16; // Range check strict upper bound

impl<F: RichField, const D: usize> RangeCheckStark<F, D> {
    const BASE: usize = 1 << 16;
}

impl<F: RichField + Extendable<D>, const D: usize> Stark<F, D> for RangeCheckStark<F, D> {
    const COLUMNS: usize = COL_NUM_RC;
    // TODO: add PUBLIC_INPUTS
    const PUBLIC_INPUTS: usize = 0;

    // Split U32 into 2 16bit limbs
    // Sumcheck between Val and limbs
    // RC for limbs
    fn eval_packed_generic<FE, P, const D2: usize>(
        &self,
        vars: StarkEvaluationVars<FE, P, { Self::COLUMNS }, { Self::PUBLIC_INPUTS }>,
        yield_constr: &mut ConstraintConsumer<P>,
    ) where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>,
    {
        let val = vars.local_values[columns::VAL];
        let limb_lo = vars.local_values[columns::LIMB_LO];
        let limb_hi = vars.local_values[columns::LIMB_HI];

        // Addition check for op0, op1, diff
        let base = P::Scalar::from_canonical_usize(Self::BASE);
        let sum = limb_lo + limb_hi * base;

        yield_constr.constraint(val - sum);

        // eval_lookups(
        //     vars,
        //     yield_constr,
        //     columns::LIMB_LO_PERMUTED,
        //     columnsFIX_RANGE_CHECK_U16_PERMUTED_LO,
        // );
        // eval_lookups(
        //     vars,
        //     yield_constr,
        //     LIMB_HI_PERMUTED,
        //     FIX_RANGE_CHECK_U16_PERMUTED_HI,
        // );
    }

    fn eval_ext_circuit(
        &self,
        builder: &mut CircuitBuilder<F, D>,
        vars: StarkEvaluationTargets<D, { Self::COLUMNS }, { Self::PUBLIC_INPUTS }>,
        yield_constr: &mut RecursiveConstraintConsumer<F, D>,
    ) {
        let val = vars.local_values[columns::VAL];
        let limb_lo = vars.local_values[columns::LIMB_LO];
        let limb_hi = vars.local_values[columns::LIMB_HI];

        // Addition check for op0, op1, diff
        let base = builder.constant_extension(F::Extension::from_canonical_usize(Self::BASE));
        let sum = builder.mul_add_extension(limb_hi, base, limb_lo);
        let val_sum_diff = builder.sub_extension(val, sum);
        yield_constr.constraint(builder, val_sum_diff);

        // TODO: add lookups
        // eval_lookups_circuit(
        //     builder,
        //     vars,
        //     yield_constr,
        //     LIMB_LO_PERMUTED,
        //     FIX_RANGE_CHECK_U16_PERMUTED_LO,
        // );
        // eval_lookups_circuit(
        //     builder,
        //     vars,
        //     yield_constr,
        //     LIMB_HI_PERMUTED,
        //     FIX_RANGE_CHECK_U16_PERMUTED_HI,
        // );
    }

    fn constraint_degree(&self) -> usize {
        3
    }
}

mod tests {
    use anyhow::Result;
    use plonky2::plonk::config::{GenericConfig, PoseidonGoldilocksConfig};
    use starky::stark_testing::test_stark_low_degree;

    use crate::rangecheck::rangecheck_stark::RangeCheckStark;

    #[test]
    fn degree() -> Result<()> {
        const D: usize = 2;
        type C = PoseidonGoldilocksConfig;
        type F = <C as GenericConfig<D>>::F;
        type S = RangeCheckStark<F, D>;

        let stark = S {
            f: Default::default(),
        };
        test_stark_low_degree(stark)
    }

    // TODO: test against some dummy traces
    #[test]
    fn basic_trace() {}
}

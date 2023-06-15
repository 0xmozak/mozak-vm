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

use super::columns;
use crate::lookup::{eval_lookups, eval_lookups_circuit};

#[derive(Copy, Clone, Default)]
pub struct RangeCheckStark<F, const D: usize> {
    pub _f: PhantomData<F>,
}

/// Constrain val - (limb_hi ** base + limb_lo) == 0
fn constrain_value<P: PackedField>(
    base: P::Scalar,
    local_values: &[P; columns::NUM_RC_COLS],
    yield_constr: &mut ConstraintConsumer<P>,
) {
    let val = local_values[columns::VAL];
    let limb_lo = local_values[columns::LIMB_LO];
    let limb_hi = local_values[columns::LIMB_HI];
    yield_constr.constraint(val - (limb_lo + limb_hi * base));
}

impl<F: RichField, const D: usize> RangeCheckStark<F, D> {
    const BASE: usize = 1 << 16;
}

impl<F: RichField + Extendable<D>, const D: usize> Stark<F, D> for RangeCheckStark<F, D> {
    const COLUMNS: usize = columns::NUM_RC_COLS;
    const PUBLIC_INPUTS: usize = 0;

    /// Given the u32 value and the u16 limbs found in our variables to be
    /// evaluated, perform:
    ///   1. sumcheck between val (u32) and limbs (u16),
    ///   2. rangecheck for limbs.
    fn eval_packed_generic<FE, P, const D2: usize>(
        &self,
        vars: StarkEvaluationVars<FE, P, { Self::COLUMNS }, { Self::PUBLIC_INPUTS }>,
        yield_constr: &mut ConstraintConsumer<P>,
    ) where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>,
    {
        constrain_value(
            P::Scalar::from_canonical_usize(Self::BASE),
            vars.local_values,
            yield_constr,
        );
        eval_lookups(
            vars,
            yield_constr,
            columns::LIMB_LO_PERMUTED,
            columns::FIXED_RANGE_CHECK_U16_PERMUTED_LO,
        );
        eval_lookups(
            vars,
            yield_constr,
            columns::LIMB_HI_PERMUTED,
            columns::FIXED_RANGE_CHECK_U16_PERMUTED_HI,
        );
    }

    /// Given the u32 value and the u16 limbs found in our variables to be
    /// evaluated, perform:
    ///   1. sumcheck between val (u32) and limbs (u16),
    ///   2. rangecheck for limbs.
    fn eval_ext_circuit(
        &self,
        builder: &mut CircuitBuilder<F, D>,
        vars: StarkEvaluationTargets<D, { Self::COLUMNS }, { Self::PUBLIC_INPUTS }>,
        yield_constr: &mut RecursiveConstraintConsumer<F, D>,
    ) {
        let val = vars.local_values[columns::VAL];
        let limb_lo = vars.local_values[columns::LIMB_LO];
        let limb_hi = vars.local_values[columns::LIMB_HI];

        let base = builder.constant_extension(F::Extension::from_canonical_usize(Self::BASE));
        let sum = builder.mul_add_extension(limb_hi, base, limb_lo);
        let val_sum_diff = builder.sub_extension(val, sum);
        yield_constr.constraint(builder, val_sum_diff);

        eval_lookups_circuit(
            builder,
            vars,
            yield_constr,
            columns::LIMB_LO_PERMUTED,
            columns::FIXED_RANGE_CHECK_U16_PERMUTED_LO,
        );
        eval_lookups_circuit(
            builder,
            vars,
            yield_constr,
            columns::LIMB_HI_PERMUTED,
            columns::FIXED_RANGE_CHECK_U16_PERMUTED_HI,
        );
    }

    fn constraint_degree(&self) -> usize {
        3
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use log::trace;
    use mozak_vm::test_utils::simple_test;
    use plonky2::{
        field::{goldilocks_field::GoldilocksField, types::Sample},
        plonk::config::{GenericConfig, PoseidonGoldilocksConfig},
        util::log2_strict,
    };
    use starky::{
        stark::Stark,
        stark_testing::{test_stark_circuit_constraints, test_stark_low_degree},
    };

    use super::*;
    use crate::generation::rangecheck::generate_rangecheck_trace;

    const D: usize = 2;
    type C = PoseidonGoldilocksConfig;
    type F = <C as GenericConfig<D>>::F;
    type S = RangeCheckStark<F, D>;

    fn generate_failing_trace() -> [Vec<GoldilocksField>; columns::NUM_RC_COLS] {
        let (rows, _) = simple_test(
            4,
            &[(0_u32, 0x0073_02b3 /* add r5, r6, r7 */)],
            &[(6, 100), (7, 100)],
        );
        let mut trace = generate_rangecheck_trace::<F>(&rows);
        trace[0][columns::VAL] = GoldilocksField(u64::from(u32::MAX) + 1_u64);
        trace
    }

    #[test]
    fn test_degree() -> Result<()> {
        let stark = S::default();
        test_stark_low_degree(stark)
    }

    #[test]
    fn test_rangecheck_stark_circuit() -> Result<()> {
        let stark = S::default();
        test_stark_circuit_constraints::<F, C, S, D>(stark)
    }

    #[test]
    fn test_rangecheck_stark() {
        let stark = S::default();
        let (rows, _) = simple_test(
            4,
            &[(0_u32, 0x0073_02b3 /* add r5, r6, r7 */)],
            &[(6, 100), (7, 100)],
        );

        let trace = generate_rangecheck_trace::<F>(&rows);

        let len = trace[0].len();
        let last = F::primitive_root_of_unity(log2_strict(len)).inverse();
        let subgroup =
            F::cyclic_subgroup_known_order(F::primitive_root_of_unity(log2_strict(len)), len);

        for i in 0..1 {
            let local_values = trace
                .iter()
                .map(|row| row[i % len])
                .collect::<Vec<_>>()
                .try_into()
                .unwrap();
            let next_values = trace
                .iter()
                .map(|row| row[(i + 1) % len])
                .collect::<Vec<_>>()
                .try_into()
                .unwrap();

            let vars = StarkEvaluationVars {
                local_values: &local_values,
                next_values: &next_values,
                public_inputs: &[],
            };

            let mut constraint_consumer = ConstraintConsumer::new(
                vec![F::rand()],
                subgroup[i] - last,
                if i == 0 {
                    GoldilocksField::ONE
                } else {
                    GoldilocksField::ZERO
                },
                if i == len - 1 {
                    GoldilocksField::ONE
                } else {
                    GoldilocksField::ZERO
                },
            );
            stark.eval_packed_generic(vars, &mut constraint_consumer);

            for &acc in &constraint_consumer.constraint_accs {
                if !acc.eq(&GoldilocksField::ZERO) {
                    trace!("constraint error in line {i}");
                }
                assert_eq!(acc, GoldilocksField::ZERO);
            }
        }
    }

    #[test]
    fn test_rangecheck_stark_big_trace() {
        let stark = S::default();
        let inst = 0x0073_02b3 /* add r5, r6, r7 */;

        let mut mem = vec![];
        for i in 0..u16::MAX as u32 + 1 {
            mem.push((u32::from(i as u32 * 4), inst));
        }
        let (rows, _) = simple_test(4, &mem, &[(6, 100), (7, 100)]);

        let trace = generate_rangecheck_trace::<F>(&rows);
        println!("trace len: {}", trace[0].len());

        let len = trace[0].len();
        let last = F::primitive_root_of_unity(log2_strict(len)).inverse();
        let subgroup =
            F::cyclic_subgroup_known_order(F::primitive_root_of_unity(log2_strict(len)), len);

        for i in 0..1 {
            let local_values = trace
                .iter()
                .map(|row| row[i % len])
                .collect::<Vec<_>>()
                .try_into()
                .unwrap();
            let next_values = trace
                .iter()
                .map(|row| row[(i + 1) % len])
                .collect::<Vec<_>>()
                .try_into()
                .unwrap();

            let vars = StarkEvaluationVars {
                local_values: &local_values,
                next_values: &next_values,
                public_inputs: &[],
            };

            let mut constraint_consumer = ConstraintConsumer::new(
                vec![F::rand()],
                subgroup[i] - last,
                if i == 0 {
                    GoldilocksField::ONE
                } else {
                    GoldilocksField::ZERO
                },
                if i == len - 1 {
                    GoldilocksField::ONE
                } else {
                    GoldilocksField::ZERO
                },
            );
            stark.eval_packed_generic(vars, &mut constraint_consumer);

            for &acc in &constraint_consumer.constraint_accs {
                if !acc.eq(&GoldilocksField::ZERO) {
                    trace!("constraint error in line {i}");
                }
                assert_eq!(acc, GoldilocksField::ZERO);
            }
        }
    }

    #[test]
    fn test_rangecheck_stark_fail() {
        let stark = S::default();
        let trace = generate_failing_trace();

        let len = trace[0].len();
        let last = F::primitive_root_of_unity(log2_strict(len)).inverse();
        let subgroup =
            F::cyclic_subgroup_known_order(F::primitive_root_of_unity(log2_strict(len)), len);

        for i in 0..len {
            let local_values = trace
                .iter()
                .map(|row| row[i % len])
                .collect::<Vec<_>>()
                .try_into()
                .unwrap();
            let next_values = trace
                .iter()
                .map(|row| row[(i + 1) % len])
                .collect::<Vec<_>>()
                .try_into()
                .unwrap();

            let vars = StarkEvaluationVars {
                local_values: &local_values,
                next_values: &next_values,
                public_inputs: &[],
            };

            let mut constraint_consumer = ConstraintConsumer::new(
                vec![F::rand()],
                subgroup[i] - last,
                if i == 0 {
                    GoldilocksField::ONE
                } else {
                    GoldilocksField::ZERO
                },
                if i == len - 1 {
                    GoldilocksField::ONE
                } else {
                    GoldilocksField::ZERO
                },
            );
            stark.eval_packed_generic(vars, &mut constraint_consumer);

            for &acc in &constraint_consumer.constraint_accs {
                if i == 0 {
                    assert_ne!(acc, GoldilocksField::ZERO);
                } else {
                    if !acc.eq(&GoldilocksField::ZERO) {
                        trace!("constraint error in line {i}");
                    }

                    assert_eq!(acc, GoldilocksField::ZERO);
                }
            }
        }
    }
}

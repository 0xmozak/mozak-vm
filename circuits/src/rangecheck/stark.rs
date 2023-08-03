use std::borrow::Borrow;
use std::marker::PhantomData;

use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::hash::hash_types::RichField;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};
use starky::permutation::PermutationPair;
use starky::stark::Stark;
use starky::vars::{StarkEvaluationTargets, StarkEvaluationVars};

use super::columns;
use super::columns::{RangeCheckColumnsView, MAP};
use crate::lookup::eval_lookups;

#[derive(Copy, Clone, Default)]
#[allow(clippy::module_name_repetitions)]
pub struct RangeCheckStark<F, const D: usize> {
    pub _f: PhantomData<F>,
}

/// Constrain `val` - (`limb_hi` ** base + `limb_lo`) == 0
fn constrain_value<P: PackedField>(
    base: P::Scalar,
    local_values: &RangeCheckColumnsView<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    let val = local_values.val;
    let limb_lo = local_values.limb_lo;
    let limb_hi = local_values.limb_hi;
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
        P: PackedField<Scalar = FE>, {
        let lv = vars.local_values.borrow();
        let nv: &RangeCheckColumnsView<P> = vars.next_values.borrow();
        constrain_value(
            P::Scalar::from_canonical_usize(Self::BASE),
            lv,
            yield_constr,
        );
        eval_lookups(
            vars,
            yield_constr,
            MAP.limb_lo_permuted,
            MAP.fixed_range_check_u16_permuted_lo,
        );
        eval_lookups(
            vars,
            yield_constr,
            MAP.limb_hi_permuted,
            MAP.fixed_range_check_u16_permuted_hi,
        );
        yield_constr.constraint_first_row(lv.fixed_range_check_u16);
        yield_constr.constraint_transition(
            (nv.fixed_range_check_u16 - lv.fixed_range_check_u16 - FE::ONE)
                * (nv.fixed_range_check_u16 - lv.fixed_range_check_u16),
        );
        yield_constr.constraint_last_row(
            lv.fixed_range_check_u16 - FE::from_canonical_u64(u64::from(u16::MAX)),
        );
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

    fn permutation_pairs(&self) -> Vec<PermutationPair> {
        vec![
            PermutationPair::singletons(MAP.limb_lo, MAP.limb_lo_permuted),
            PermutationPair::singletons(MAP.limb_hi, MAP.limb_hi_permuted),
            PermutationPair::singletons(
                MAP.fixed_range_check_u16,
                MAP.fixed_range_check_u16_permuted_lo,
            ),
            PermutationPair::singletons(
                MAP.fixed_range_check_u16,
                MAP.fixed_range_check_u16_permuted_hi,
            ),
        ]
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use log::trace;
    use mozak_vm::instruction::{Instruction, Op};
    use mozak_vm::test_utils::simple_test_code;
    use plonky2::field::goldilocks_field::GoldilocksField;
    use plonky2::field::types::{Field, Sample};
    use plonky2::plonk::config::{GenericConfig, PoseidonGoldilocksConfig};
    use plonky2::util::log2_strict;
    use starky::stark::Stark;
    use starky::stark_testing::test_stark_low_degree;

    use super::*;
    use crate::generation::cpu::generate_cpu_trace;
    use crate::generation::rangecheck::generate_rangecheck_trace;

    const D: usize = 2;
    type C = PoseidonGoldilocksConfig;
    type F = <C as GenericConfig<D>>::F;
    type S = RangeCheckStark<F, D>;

    /// Generates a trace which contains a value that should fail the range
    /// check.
    fn generate_failing_trace() -> [Vec<GoldilocksField>; columns::NUM_RC_COLS] {
        let record = simple_test_code(
            &[Instruction::new(Op::ADD, 5, 6, 7, 0)],
            &[],
            // Use values that would become limbs later
            &[(6, 0xffff), (7, 0xffff)],
        );

        let cpu_trace = generate_cpu_trace::<F>(&record.executed);
        let mut trace = generate_rangecheck_trace::<F>(&cpu_trace);
        // Manually alter the value here to be larger than a u32.
        trace[0][MAP.val] = GoldilocksField(u64::from(u32::MAX) + 1_u64);
        trace
    }

    #[test]
    fn test_degree() -> Result<()> {
        let stark = S::default();
        test_stark_low_degree(stark)
    }

    #[test]
    fn test_rangecheck_stark_big_trace() {
        let stark = S::default();
        let inst = 0x0073_02b3 /* add r5, r6, r7 */;

        let mut mem = vec![];
        let u16max = u32::from(u16::MAX);
        for i in 0..=u16max {
            mem.push((i * 4, inst));
        }
        let record = simple_test_code(&[Instruction::new(Op::ADD, 5, 6, 7, 0)], &[], &[
            (6, 100),
            (7, 100),
        ]);

        let cpu_rows = generate_cpu_trace::<F>(&record.executed);
        let trace = generate_rangecheck_trace::<F>(&cpu_rows);

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

        let local_values = trace
            .iter()
            .map(|row| row[0])
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();
        let next_values = trace
            .iter()
            .map(|row| row[1])
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
            subgroup[0] - last,
            GoldilocksField::ONE,
            GoldilocksField::ZERO,
        );
        stark.eval_packed_generic(vars, &mut constraint_consumer);

        // Constraint should not hold, since trace contains a value > u16::MAX.
        assert_ne!(
            constraint_consumer.constraint_accs[0],
            GoldilocksField::ZERO
        );
    }
}

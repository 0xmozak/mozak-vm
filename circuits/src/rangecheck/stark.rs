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
        let lv: &RangeCheckColumnsView<P> = vars.local_values.borrow();
        let nv: &RangeCheckColumnsView<P> = vars.next_values.borrow();

        // Check: the limbs are in range of 0..2^16 using lookup tables.
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

        // Check: the `fixed_range_check_u16` forms a sequence from 0 to 2^16-1.
        //  this column will be used to connect to the permutation columns,
        //  `fixed_range_check_u16_permuted_hi` and `fixed_range_check_u16_permuted_lo`.
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
    use mozak_vm::instruction::{Args, Instruction, Op};
    use mozak_vm::test_utils::simple_test_code;
    use plonky2::field::goldilocks_field::GoldilocksField;
    use plonky2::field::types::Field;
    use plonky2::plonk::config::{GenericConfig, PoseidonGoldilocksConfig};
    use starky::stark::Stark;
    use starky::stark_testing::test_stark_low_degree;

    use super::*;
    use crate::generation::cpu::generate_cpu_trace;
    use crate::generation::memory::generate_memory_trace;
    use crate::generation::rangecheck::generate_rangecheck_trace;
    use crate::rangecheck::columns::NUM_RC_COLS;

    const D: usize = 2;
    type C = PoseidonGoldilocksConfig;
    type F = <C as GenericConfig<D>>::F;
    type S = RangeCheckStark<F, D>;

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
        let (program, record) = simple_test_code(
            &[Instruction {
                op: Op::ADD,
                args: Args {
                    rd: 5,
                    rs1: 6,
                    rs2: 7,
                    ..Args::default()
                },
            }],
            &[],
            &mem,
            &[(6, 100), (7, 100)],
        );

        let cpu_rows = generate_cpu_trace::<F>(&program, &record);
        let memory_trace = generate_memory_trace::<F>(&program, &record.executed);
        let trace = generate_rangecheck_trace::<F>(&cpu_rows, &memory_trace);

        let len = trace[0].len();

        for i in 0..len {
            let local_values = trace
                .iter()
                .map(|row| row[i])
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

            let mut constraint_consumer = ConstraintConsumer::new_debug_api(i == 0, i == len - 1);
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
    fn test_rangecheck_stark_fails_range_constraint() {
        let stark = S::default();
        let (program, record) = simple_test_code(
            &[Instruction {
                op: Op::ADD,
                args: Args {
                    rd: 5,
                    rs1: 6,
                    rs2: 7,
                    ..Args::default()
                },
            }],
            &[],
            &[],
            &[],
        );

        let cpu_trace = generate_cpu_trace::<F>(&program, &record);
        let memory_trace = generate_memory_trace::<F>(&program, &record.executed);
        let mut trace = generate_rangecheck_trace::<F>(&cpu_trace, &memory_trace);

        let len = trace[0].len();
        let bad_row_idx = len - 1;

        // Set limb to be larger than u16::MAX, to fail the range check.
        trace[MAP.limb_lo_permuted][bad_row_idx] = F::from_canonical_u32(u32::from(u16::MAX) + 1);

        let local_values: [GoldilocksField; NUM_RC_COLS] = trace
            .iter()
            .map(|row| row[bad_row_idx - 1])
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();
        let next_values = trace
            .iter()
            // We want the next values to be the last row, since our constraints
            // that asserts the horizontal diff described in Halo2 act on next
            // values, not the local values.
            .map(|row| row[bad_row_idx])
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();

        let vars = StarkEvaluationVars {
            local_values: &local_values,
            next_values: &next_values,
            public_inputs: &[],
        };

        let mut constraint_consumer = ConstraintConsumer::new_debug_api(false, true);
        stark.eval_packed_generic(vars, &mut constraint_consumer);

        // If this evaluates to true, this should mean that our range check failed.
        // Note that it is impossible for our sumcheck to fail since that constraint
        // is based on unrelated columns from what we tweaked above.
        assert_ne!(
            constraint_consumer.constraint_accs[0],
            GoldilocksField::ZERO
        );
    }
}

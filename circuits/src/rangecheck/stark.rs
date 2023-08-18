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
        let lv: &RangeCheckColumnsView<P> = vars.local_values.borrow();
        let nv: &RangeCheckColumnsView<P> = vars.next_values.borrow();

        // Check: the value is built from two limbs.
        // And then check that the limbs are in range of 0..2^16 using lookup tables.
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
    use plonky2::field::types::{Field, PrimeField64, Sample};
    use plonky2::plonk::config::{GenericConfig, PoseidonGoldilocksConfig};
    use starky::stark::Stark;
    use starky::stark_testing::test_stark_low_degree;

    use super::*;
    use crate::generation::cpu::generate_cpu_trace;
    use crate::generation::rangecheck::{
        generate_rangecheck_trace, limbs_from_u32, RANGE_CHECK_U16_SIZE,
    };
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
            &mem,
            &[(6, 100), (7, 100)],
        );

        let cpu_rows = generate_cpu_trace::<F>(&program, &record);
        let trace = generate_rangecheck_trace::<F>(&cpu_rows);

        let len = trace[0].len();

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
                if i == len - 1 {
                    GoldilocksField::ZERO
                } else {
                    GoldilocksField::ONE
                },
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
        );

        let cpu_trace = generate_cpu_trace::<F>(&program, &record);
        let mut trace = generate_rangecheck_trace::<F>(&cpu_trace);

        // The above generations setup the traces nicely, but we need to introduce
        // malicious entries here to test our failing cases.
        let out_of_range_value = F::from_canonical_u32(u32::from(u16::MAX) + 1);
        trace[MAP.val][0] = out_of_range_value;
        trace[MAP.limb_hi][0] = F::ZERO;
        // We introduce a value bigger than `u16::MAX`) in the
        // lower limb.
        trace[MAP.limb_lo][0] = out_of_range_value;
        trace[MAP.fixed_range_check_u16_permuted_lo][0] = out_of_range_value;

        let local_values: [GoldilocksField; NUM_RC_COLS] = trace
            .iter()
            .map(|row| *row.last().unwrap())
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();
        let next_values = trace
            .iter()
            // The next row after the last row wraps around.
            .map(|row| row[0])
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
            F::ZERO,
            GoldilocksField::ZERO,
            GoldilocksField::ONE,
        );
        stark.eval_packed_generic(vars, &mut constraint_consumer);

        // Manually check sum constraint to be sure that our constraints hold for the
        // sum check.
        assert_eq!(
            trace[MAP.val][0],
            trace[MAP.limb_hi][0] * F::from_canonical_usize(RangeCheckStark::<F, D>::BASE)
                + trace[MAP.limb_lo][0]
        );
        // If the above assert passes and the below condition fails i.e. assert_ne
        // evaluates to true, this should mean that our range check failed.
        assert_ne!(
            constraint_consumer.constraint_accs[0],
            GoldilocksField::ZERO
        );
    }

    #[test]
    fn test_rangecheck_stark_fails_sum_constraint() {
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
        );

        let cpu_trace = generate_cpu_trace::<F>(&program, &record);
        let mut trace = generate_rangecheck_trace::<F>(&cpu_trace);
        // The above generations setup the traces nicely, but we need to introduce
        // a malicious entry here to test our failing case.
        let value: u32 = 0xDEAD_BEEF;
        let (limb_hi, limb_lo): (F, F) = limbs_from_u32(value);
        trace[MAP.val][0] = GoldilocksField(value.into());
        trace[MAP.limb_hi][0] = limb_hi;
        // Subtract one intentionally to make our sum check constraint fail.
        let malicious_limb_lo = limb_lo - F::ONE;
        trace[MAP.limb_lo][0] = malicious_limb_lo;

        let local_values: [GoldilocksField; NUM_RC_COLS] = trace
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
            GoldilocksField::ONE,
            GoldilocksField::ONE,
            GoldilocksField::ZERO,
        );
        stark.eval_packed_generic(vars, &mut constraint_consumer);

        // Manually check range constraint to be sure that our constraints hold for the
        // range check.
        assert!((0..RANGE_CHECK_U16_SIZE)
            .contains(&usize::try_from(F::to_noncanonical_u64(&trace[MAP.limb_lo][0])).unwrap()));
        assert!((0..RANGE_CHECK_U16_SIZE)
            .contains(&usize::try_from(F::to_noncanonical_u64(&trace[MAP.limb_hi][0])).unwrap()));
        // If the above assert passes and the below condition fails i.e. assert_ne
        // evaluates to true, this should mean that our sum constraint failed.
        assert_ne!(
            constraint_consumer.constraint_accs[0],
            GoldilocksField::ZERO
        );
    }
}

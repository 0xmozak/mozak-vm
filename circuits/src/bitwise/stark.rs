use std::borrow::Borrow;
use std::marker::PhantomData;

use itertools::{chain, iproduct, izip};
use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::hash::hash_types::RichField;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::plonk_common::reduce_with_powers;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};
use starky::permutation::PermutationPair;
use starky::stark::Stark;
use starky::vars::{StarkEvaluationTargets, StarkEvaluationVars};

use super::columns::{BitwiseColumnsView, BASE, MAP};
use crate::columns_view::NumberOfColumns;
use crate::lookup::eval_lookups;

#[derive(Clone, Copy, Default)]
#[allow(clippy::module_name_repetitions)]
pub struct BitwiseStark<F, const D: usize> {
    pub _f: PhantomData<F>,
}

impl<F: RichField + Extendable<D>, const D: usize> Stark<F, D> for BitwiseStark<F, D> {
    const COLUMNS: usize = BitwiseColumnsView::<F>::NUMBER_OF_COLUMNS;
    const PUBLIC_INPUTS: usize = 0;

    fn eval_packed_generic<FE, P, const D2: usize>(
        &self,
        vars: StarkEvaluationVars<FE, P, { Self::COLUMNS }, { Self::PUBLIC_INPUTS }>,
        yield_constr: &mut ConstraintConsumer<P>,
    ) where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>, {
        let lv: &BitwiseColumnsView<_> = vars.local_values.borrow();
        let nv: &BitwiseColumnsView<_> = vars.next_values.borrow();
        let e = &lv.execution;

        // check limbs sum to our given value.
        // We interpret limbs as digits in base 256 == 2**8.
        for (opx, opx_limbs) in [
            (e.op1, e.op1_limbs),
            (e.op2, e.op2_limbs),
            (e.res, e.res_limbs),
        ] {
            yield_constr.constraint(
                reduce_with_powers(&opx_limbs, P::Scalar::from_noncanonical_u64(256_u64)) - opx,
            );
        }

        // Constraint fix_range_check_u8.
        yield_constr.constraint_first_row(lv.fixed_range_check_u8);
        yield_constr.constraint_transition(
            (nv.fixed_range_check_u8 - lv.fixed_range_check_u8 - FE::ONE)
                * (nv.fixed_range_check_u8 - lv.fixed_range_check_u8),
        );
        yield_constr.constraint_last_row(
            lv.fixed_range_check_u8 - FE::from_canonical_u64(u64::from(u8::MAX)),
        );

        // Constrain compression logic.
        let base = FE::from_noncanonical_u64(BASE.into());
        for (op1_limb, op2_limb, res_limb, compressed_limb) in
            izip!(e.op1_limbs, e.op2_limbs, e.res_limbs, lv.compressed_limbs)
        {
            yield_constr.constraint(
                reduce_with_powers(&[op1_limb, op2_limb, res_limb], base) - compressed_limb,
            );
        }

        // TODO(Matthias): Once we fix `eval_lookups` we can probably drop the MAP
        // here.
        for (fixed_range_check_u8_permuted, opx_limbs_permuted) in izip!(
            MAP.fixed_range_check_u8_permuted,
            chain!(
                MAP.op1_limbs_permuted,
                MAP.op2_limbs_permuted,
                MAP.res_limbs_permuted
            )
        ) {
            eval_lookups(
                vars,
                yield_constr,
                opx_limbs_permuted,
                fixed_range_check_u8_permuted,
            );
        }

        for (fixed_compressed_permuted, compressed_permuted) in
            izip!(MAP.fixed_compressed_permuted, MAP.compressed_permuted)
        {
            eval_lookups(
                vars,
                yield_constr,
                compressed_permuted,
                fixed_compressed_permuted,
            );
        }
    }

    fn constraint_degree(&self) -> usize { 3 }

    fn permutation_pairs(&self) -> Vec<PermutationPair> {
        chain!(
            izip!(MAP.compressed_limbs, MAP.compressed_permuted),
            iproduct!([MAP.fixed_compressed], MAP.fixed_compressed_permuted)
        )
        .map(|(a, b)| PermutationPair::singletons(a, b))
        .collect()
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
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use mozak_vm::instruction::{Args, Instruction, Op};
    use mozak_vm::test_utils::simple_test_code;
    use plonky2::util::timing::TimingTree;
    use starky::prover::prove as prove_table;
    use starky::stark_testing::test_stark_low_degree;
    use starky::verifier::verify_stark_proof;

    use crate::bitwise::stark::BitwiseStark;
    use crate::generation::bitwise::generate_bitwise_trace;
    use crate::generation::cpu::generate_cpu_trace;
    use crate::stark::utils::trace_to_poly_values;
    use crate::test_utils::{standard_faster_config, C, D, F};

    type S = BitwiseStark<F, D>;
    #[test]
    fn test_degree() -> Result<()> {
        let stark = S::default();
        test_stark_low_degree(stark)
    }

    fn test_bitwise_stark(a: u32, b: u32, imm: u32) {
        let config = standard_faster_config();

        let record = simple_test_code(
            &[
                Instruction {
                    op: Op::XOR,
                    args: Args {
                        rs1: 5,
                        rs2: 6,
                        rd: 7,
                        imm,
                        ..Args::default()
                    },
                },
                Instruction {
                    op: Op::AND,
                    args: Args {
                        rs1: 5,
                        rs2: 6,
                        rd: 7,
                        imm,
                        ..Args::default()
                    },
                },
                Instruction {
                    op: Op::OR,
                    args: Args {
                        rs1: 5,
                        rs2: 6,
                        rd: 7,
                        imm,
                        ..Args::default()
                    },
                },
            ],
            &[],
            &[(5, a), (6, b)],
        );
        // assert_eq!(record.last_state.get_register_value(7), a ^ (b + imm));
        let cpu_trace = generate_cpu_trace(&record.executed);
        let trace = generate_bitwise_trace(&record.executed, &cpu_trace);
        let trace_poly_values = trace_to_poly_values(trace);
        let stark = S::default();

        let proof = prove_table::<F, C, S, D>(
            stark,
            &config,
            trace_poly_values,
            [],
            &mut TimingTree::default(),
        )
        .unwrap();
        verify_stark_proof(stark, proof, &config).unwrap();
    }
    use proptest::prelude::{any, ProptestConfig};
    use proptest::proptest;
    proptest! {
            #![proptest_config(ProptestConfig::with_cases(4))]
            #[test]
            fn prove_bitwise_immediate_proptest(a in any::<u32>(), b in any::<u32>()) {
                test_bitwise_stark(a, 0, b);
            }
            #[test]
            fn prove_bitwise_proptest(a in any::<u32>(), b in any::<u32>()) {
                test_bitwise_stark(a, b, 0);
            }
    }
}

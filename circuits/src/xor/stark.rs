use std::borrow::Borrow;
use std::marker::PhantomData;

use itertools::{chain, izip};
use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::hash::hash_types::RichField;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::plonk_common::reduce_with_powers;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};
use starky::stark::Stark;
use starky::vars::{StarkEvaluationTargets, StarkEvaluationVars};

use super::columns::XorColumnsView;
use crate::columns_view::NumberOfColumns;

#[derive(Clone, Copy, Default)]
#[allow(clippy::module_name_repetitions)]
pub struct XorStark<F, const D: usize> {
    pub _f: PhantomData<F>,
}

impl<F: RichField + Extendable<D>, const D: usize> Stark<F, D> for XorStark<F, D> {
    const COLUMNS: usize = XorColumnsView::<F>::NUMBER_OF_COLUMNS;
    const PUBLIC_INPUTS: usize = 0;

    fn eval_packed_generic<FE, P, const D2: usize>(
        &self,
        vars: StarkEvaluationVars<FE, P, { Self::COLUMNS }, { Self::PUBLIC_INPUTS }>,
        yield_constr: &mut ConstraintConsumer<P>,
    ) where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>, {
        let lv: &XorColumnsView<_> = vars.local_values.borrow();

        // Each limb must be a either 0 or 1.
        for bit_value in chain!(lv.limbs.a, lv.limbs.b, lv.limbs.out) {
            yield_constr.constraint(bit_value * (bit_value - P::ONES));
        }

        // Check limbs sum to our given value.
        // We interpret limbs as digits in base 2.
        for (opx, opx_limbs) in izip![lv.execution, lv.limbs] {
            yield_constr.constraint(reduce_with_powers(&opx_limbs, P::Scalar::TWO) - opx);
        }

        for (a, b, res) in izip!(lv.limbs.a, lv.limbs.b, lv.limbs.out) {
            // For two binary digits a and b, we want to compute a ^ b.
            // Conventiently, adding with carry gives:
            // a + b == (a & b, a ^ b) == 2 * (a & b) + (a ^ b)
            // Solving for (a ^ b) gives:
            // (a ^ b) := a + b - 2 * (a & b) == a + b - 2 * a * b
            let xor = (a + b) - (a * b).doubles();
            yield_constr.constraint(res - xor);
        }
    }

    fn constraint_degree(&self) -> usize { 3 }

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
    use plonky2::timed;
    use plonky2::util::timing::TimingTree;
    use starky::prover::prove as prove_table;
    use starky::stark_testing::test_stark_low_degree;
    use starky::verifier::verify_stark_proof;

    use crate::generation::bitwise::generate_bitwise_trace;
    use crate::generation::cpu::generate_cpu_trace;
    use crate::stark::utils::trace_rows_to_poly_values;
    use crate::test_utils::{standard_faster_config, C, D, F};
    use crate::xor::stark::XorStark;

    type S = XorStark<F, D>;
    #[test]
    fn test_degree() -> Result<()> {
        let stark = S::default();
        test_stark_low_degree(stark)
    }

    fn test_xor_stark(a: u32, b: u32, imm: u32) {
        let config = standard_faster_config();

        let (program, record) = simple_test_code(
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
        let mut timing = TimingTree::new("xor", log::Level::Debug);
        let cpu_trace = generate_cpu_trace(&program, &record);
        let trace = timed!(
            timing,
            "generate_bitwise_trace",
            generate_bitwise_trace(&cpu_trace)
        );
        let trace_poly_values = timed!(timing, "trace to poly", trace_rows_to_poly_values(trace));
        let stark = S::default();

        let proof = timed!(
            timing,
            "xor proof",
            prove_table::<F, C, S, D>(stark, &config, trace_poly_values, [], &mut timing,)
        );
        let proof = proof.unwrap();
        let verification_res = timed!(
            timing,
            "xor verification",
            verify_stark_proof(stark, proof, &config)
        );
        verification_res.unwrap();
        timing.print();
    }
    use proptest::prelude::{any, ProptestConfig};
    use proptest::proptest;
    proptest! {
            #![proptest_config(ProptestConfig::with_cases(4))]
            #[test]
            fn prove_xor_immediate_proptest(a in any::<u32>(), b in any::<u32>()) {
                test_xor_stark(a, 0, b);
            }
            #[test]
            fn prove_xor_proptest(a in any::<u32>(), b in any::<u32>()) {
                test_xor_stark(a, b, 0);
            }
    }
}

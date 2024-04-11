use std::marker::PhantomData;

use itertools::{chain, izip};
use mozak_circuits_derive::StarkNameDisplay;
use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::plonk_common::{reduce_with_powers, reduce_with_powers_ext_circuit};
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};
use starky::evaluation_frame::{StarkEvaluationFrame, StarkFrame};
use starky::stark::Stark;

use super::columns::XorColumnsView;
use crate::columns_view::{HasNamedColumns, NumberOfColumns};
use crate::stark::utils::{is_binary, is_binary_ext_circuit};

#[derive(Clone, Copy, Default, StarkNameDisplay)]
#[allow(clippy::module_name_repetitions)]
pub struct XorStark<F, const D: usize> {
    pub _f: PhantomData<F>,
}

impl<F, const D: usize> HasNamedColumns for XorStark<F, D> {
    type Columns = XorColumnsView<F>;
}

const COLUMNS: usize = XorColumnsView::<()>::NUMBER_OF_COLUMNS;
const PUBLIC_INPUTS: usize = 0;

impl<F: RichField + Extendable<D>, const D: usize> Stark<F, D> for XorStark<F, D> {
    type EvaluationFrame<FE, P, const D2: usize> = StarkFrame<P, P::Scalar, COLUMNS, PUBLIC_INPUTS>

    where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>;
    type EvaluationFrameTarget =
        StarkFrame<ExtensionTarget<D>, ExtensionTarget<D>, COLUMNS, PUBLIC_INPUTS>;

    fn eval_packed_generic<FE, P, const D2: usize>(
        &self,
        vars: &Self::EvaluationFrame<FE, P, D2>,
        yield_constr: &mut ConstraintConsumer<P>,
    ) where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>, {
        let lv: &XorColumnsView<_> = vars.get_local_values().into();

        // We first convert both input and output to bit representation
        // We then work with the bit representations to check the Xor result.

        // Check: bit representation of inputs and output contains either 0 or 1.
        for bit_value in chain!(lv.limbs.a, lv.limbs.b, lv.limbs.out) {
            is_binary(yield_constr, bit_value);
        }

        // Check: bit representation of inputs and output were generated correctly.
        for (opx, opx_limbs) in izip![lv.execution, lv.limbs] {
            yield_constr.constraint(reduce_with_powers(&opx_limbs, P::Scalar::TWO) - opx);
        }

        // Check: output bit representation is Xor of input a and b bit representations
        for (a, b, res) in izip!(lv.limbs.a, lv.limbs.b, lv.limbs.out) {
            // Note that if a, b are in {0, 1}: (a ^ b) = a + b - 2 * a * b
            // One can check by substituting the values, that:
            //      if a = b = 0            -> 0 + 0 - 2 * 0 * 0 = 0
            //      if only a = 1 or b = 1  -> 1 + 0 - 2 * 1 * 0 = 1
            //      if a = b = 1            -> 1 + 1 - 2 * 1 * 1 = 0
            let xor = (a + b) - (a * b).doubles();
            yield_constr.constraint(res - xor);
        }
    }

    fn constraint_degree(&self) -> usize { 3 }

    fn eval_ext_circuit(
        &self,
        builder: &mut CircuitBuilder<F, D>,
        vars: &Self::EvaluationFrameTarget,
        yield_constr: &mut RecursiveConstraintConsumer<F, D>,
    ) {
        let lv: &XorColumnsView<ExtensionTarget<D>> = vars.get_local_values().into();
        for bit_value in chain!(lv.limbs.a, lv.limbs.b, lv.limbs.out) {
            is_binary_ext_circuit(builder, bit_value, yield_constr);
        }
        let two = builder.constant(F::TWO);
        for (opx, opx_limbs) in izip![lv.execution, lv.limbs] {
            let x = reduce_with_powers_ext_circuit(builder, &opx_limbs, two);
            let x_sub_opx = builder.sub_extension(x, opx);
            yield_constr.constraint(builder, x_sub_opx);
        }
        for (a, b, res) in izip!(lv.limbs.a, lv.limbs.b, lv.limbs.out) {
            let a_add_b = builder.add_extension(a, b);
            let a_mul_b = builder.mul_extension(a, b);
            let a_mul_b_doubles = builder.add_extension(a_mul_b, a_mul_b);
            let a_add_b_sub_a_mul_b_doubles = builder.sub_extension(a_add_b, a_mul_b_doubles);
            let xor = builder.sub_extension(res, a_add_b_sub_a_mul_b_doubles);
            yield_constr.constraint(builder, xor);
        }
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use mozak_runner::instruction::{Args, Instruction, Op};
    use mozak_runner::code;
    use plonky2::timed;
    use plonky2::util::timing::TimingTree;
    use starky::prover::prove as prove_table;
    use starky::stark_testing::{test_stark_circuit_constraints, test_stark_low_degree};
    use starky::verifier::verify_stark_proof;

    use crate::generation::cpu::generate_cpu_trace;
    use crate::generation::xor::generate_xor_trace;
    use crate::stark::utils::trace_rows_to_poly_values;
    use crate::test_utils::{fast_test_config, C, D, F};
    use crate::xor::stark::XorStark;

    type S = XorStark<F, D>;
    #[test]
    fn test_degree() -> Result<()> {
        let stark = S::default();
        test_stark_low_degree(stark)
    }

    fn test_xor_stark(a: u32, b: u32, imm: u32) {
        let config = fast_test_config();

        let (_program, record) = code::execute(
            [
                Instruction {
                    op: Op::XOR,
                    args: Args {
                        rs1: 5,
                        rs2: 6,
                        rd: 7,
                        imm,
                    },
                },
                Instruction {
                    op: Op::AND,
                    args: Args {
                        rs1: 5,
                        rs2: 6,
                        rd: 7,
                        imm,
                    },
                },
                Instruction {
                    op: Op::OR,
                    args: Args {
                        rs1: 5,
                        rs2: 6,
                        rd: 7,
                        imm,
                    },
                },
            ],
            &[],
            &[(5, a), (6, b)],
        );
        // assert_eq!(record.last_state.get_register_value(7), a ^ (b + imm));
        let mut timing = TimingTree::new("xor", log::Level::Debug);
        let cpu_trace = generate_cpu_trace(&record);
        let trace = timed!(timing, "generate_xor_trace", generate_xor_trace(&cpu_trace));
        let trace_poly_values = timed!(timing, "trace to poly", trace_rows_to_poly_values(trace));
        let stark = S::default();

        let proof = timed!(
            timing,
            "xor proof",
            prove_table::<F, C, S, D>(stark, &config, trace_poly_values, &[], &mut timing,)
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

    #[test]
    fn test_circuit() -> anyhow::Result<()> {
        let stark = S::default();
        test_stark_circuit_constraints::<F, C, S, D>(stark)?;

        Ok(())
    }
}

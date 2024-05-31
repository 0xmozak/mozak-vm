use std::marker::PhantomData;

use expr::{Expr, ExprBuilder, StarkFrameTyped};
use itertools::{chain, izip};
use mozak_circuits_derive::StarkNameDisplay;
use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};
use starky::evaluation_frame::StarkFrame;
use starky::stark::Stark;

use super::columns::XorColumnsView;
use crate::columns_view::{HasNamedColumns, NumberOfColumns};
use crate::expr::{build_ext, build_packed, ConstraintBuilder, GenerateConstraints};
use crate::unstark::NoColumns;

#[derive(Clone, Copy, Default, StarkNameDisplay)]
#[allow(clippy::module_name_repetitions)]
pub struct XorStark<F, const D: usize> {
    pub _f: PhantomData<F>,
}

impl<F, const D: usize> HasNamedColumns for XorStark<F, D> {
    type Columns = XorColumnsView<F>;
    type PublicInputs = NoColumns<F>;
}

const COLUMNS: usize = XorColumnsView::<()>::NUMBER_OF_COLUMNS;
const PUBLIC_INPUTS: usize = 0;

impl<'a, F, T: Copy, U, const D: usize>
    GenerateConstraints<'a, T, XorColumnsView<Expr<'a, T>>, NoColumns<U>> for XorStark<F, { D }>
{
    fn generate_constraints(
        vars: &StarkFrameTyped<XorColumnsView<Expr<'a, T>>, NoColumns<U>>,
    ) -> ConstraintBuilder<Expr<'a, T>> {
        let lv = vars.local_values;
        let mut constraints = ConstraintBuilder::default();

        // We first convert both input and output to bit representation
        // We then work with the bit representations to check the Xor result.

        // Check: bit representation of inputs and output contains either 0 or 1.
        for bit_value in chain!(lv.limbs.a, lv.limbs.b, lv.limbs.out) {
            constraints.always(bit_value.is_binary());
        }

        // Check: bit representation of inputs and output were generated correctly.
        for (opx, opx_limbs) in izip![lv.execution, lv.limbs] {
            constraints.always(Expr::reduce_with_powers(opx_limbs, 2) - opx);
        }

        // Check: output bit representation is Xor of input a and b bit representations
        for (a, b, out) in izip!(lv.limbs.a, lv.limbs.b, lv.limbs.out) {
            // Xor behaves like addition in binary field, i.e. addition with wrap-around:
            constraints.always((a + b - out) * (a + b - 2 - out));
        }

        constraints
    }
}

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
        consumer: &mut ConstraintConsumer<P>,
    ) where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>, {
        let expr_builder = ExprBuilder::default();
        let constraints = Self::generate_constraints(&expr_builder.to_typed_starkframe(vars));
        build_packed(constraints, consumer);
    }

    fn constraint_degree(&self) -> usize { 3 }

    fn eval_ext_circuit(
        &self,
        circuit_builder: &mut CircuitBuilder<F, D>,
        vars: &Self::EvaluationFrameTarget,
        consumer: &mut RecursiveConstraintConsumer<F, D>,
    ) {
        let expr_builder = ExprBuilder::default();
        let constraints = Self::generate_constraints(&expr_builder.to_typed_starkframe(vars));
        build_ext(constraints, circuit_builder, consumer);
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use mozak_runner::code;
    use mozak_runner::instruction::{Args, Instruction, Op};
    use plonky2::timed;
    use plonky2::util::timing::TimingTree;
    use starky::prover::prove as prove_table;
    use starky::stark_testing::{test_stark_circuit_constraints, test_stark_low_degree};
    use starky::verifier::verify_stark_proof;

    use crate::cpu::generation::generate_cpu_trace;
    use crate::stark::utils::trace_rows_to_poly_values;
    use crate::test_utils::{fast_test_config, C, D, F};
    use crate::xor::generation::generate_xor_trace;
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

use core::fmt::Debug;

use expr::Expr;
use itertools::{chain, izip};
use mozak_circuits_derive::StarkNameDisplay;

use super::columns::XorColumnsView;
use crate::columns_view::NumberOfColumns;
use crate::expr::{ConstraintBuilder, GenerateConstraints, StarkFrom, Vars};
use crate::unstark::NoColumns;

#[derive(Clone, Copy, Default, StarkNameDisplay)]
#[allow(clippy::module_name_repetitions)]
pub struct XorConstraints {}

#[allow(clippy::module_name_repetitions)]
pub type XorStark<F, const D: usize> =
    StarkFrom<F, XorConstraints, { D }, { COLUMNS }, { PUBLIC_INPUTS }>;

const COLUMNS: usize = XorColumnsView::<()>::NUMBER_OF_COLUMNS;
const PUBLIC_INPUTS: usize = 0;

impl GenerateConstraints<COLUMNS, PUBLIC_INPUTS> for XorConstraints {
    type PublicInputs<E: Debug> = NoColumns<E>;
    type View<E: Debug> = XorColumnsView<E>;

    fn generate_constraints<'a, T: Copy + Debug>(
        &self,
        vars: &Vars<'a, Self, T, COLUMNS, PUBLIC_INPUTS>,
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

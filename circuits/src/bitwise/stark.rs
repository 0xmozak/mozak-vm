use std::marker::PhantomData;
use std::ops::Range;

use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::hash::hash_types::RichField;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::plonk_common::reduce_with_powers;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};
use starky::stark::Stark;
use starky::vars::{StarkEvaluationTargets, StarkEvaluationVars};

use super::columns::{
    FIX_RANGE_CHECK_U8_PERMUTED, NUM_BITWISE_COL, OP1, OP1_LIMBS, OP1_LIMBS_PERMUTED, OP2,
    OP2_LIMBS, OP2_LIMBS_PERMUTED, RES, RES_LIMBS, RES_LIMBS_PERMUTED,
};
use crate::lookup::eval_lookups;
use crate::utils::from_;

#[derive(Clone, Copy, Default)]
#[allow(clippy::module_name_repetitions)]
pub struct BitwiseStark<F, const D: usize> {
    pub _f: PhantomData<F>,
}

impl<F: RichField + Extendable<D>, const D: usize> Stark<F, D> for BitwiseStark<F, D> {
    const COLUMNS: usize = NUM_BITWISE_COL;
    const PUBLIC_INPUTS: usize = 0;

    fn eval_packed_generic<FE, P, const D2: usize>(
        &self,
        vars: StarkEvaluationVars<FE, P, { Self::COLUMNS }, { Self::PUBLIC_INPUTS }>,
        yield_constr: &mut ConstraintConsumer<P>,
    ) where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>, {
        let lv = vars.local_values;

        // sumcheck for op1, op2, res limbs
        // We enforce the constraint:
        //     opx == Sum(opx_limbs * 2^(8*i))
        let mut sumcheck = |opx: usize, opx_limbs: Range<usize>| {
            let opx_limbs = lv[opx_limbs].to_vec();
            let computed_sum = reduce_with_powers(&opx_limbs, from_(1_u128 << 8));
            yield_constr.constraint(computed_sum - lv[opx]);
        };
        sumcheck(OP1, OP1_LIMBS);
        sumcheck(OP2, OP2_LIMBS);
        sumcheck(RES, RES_LIMBS);

        eval_lookups(
            vars,
            yield_constr,
            OP1_LIMBS_PERMUTED.start,
            FIX_RANGE_CHECK_U8_PERMUTED.start,
        );
        eval_lookups(
            vars,
            yield_constr,
            OP1_LIMBS_PERMUTED.start + 1,
            FIX_RANGE_CHECK_U8_PERMUTED.start + 1,
        );
        eval_lookups(
            vars,
            yield_constr,
            OP1_LIMBS_PERMUTED.start + 2,
            FIX_RANGE_CHECK_U8_PERMUTED.start + 2,
        );
        eval_lookups(
            vars,
            yield_constr,
            OP1_LIMBS_PERMUTED.start + 3,
            FIX_RANGE_CHECK_U8_PERMUTED.start + 3,
        );

        eval_lookups(
            vars,
            yield_constr,
            OP2_LIMBS_PERMUTED.start,
            FIX_RANGE_CHECK_U8_PERMUTED.start + 4,
        );
        eval_lookups(
            vars,
            yield_constr,
            OP2_LIMBS_PERMUTED.start + 1,
            FIX_RANGE_CHECK_U8_PERMUTED.start + 5,
        );
        eval_lookups(
            vars,
            yield_constr,
            OP2_LIMBS_PERMUTED.start + 2,
            FIX_RANGE_CHECK_U8_PERMUTED.start + 6,
        );
        eval_lookups(
            vars,
            yield_constr,
            OP2_LIMBS_PERMUTED.start + 3,
            FIX_RANGE_CHECK_U8_PERMUTED.start + 7,
        );

        eval_lookups(
            vars,
            yield_constr,
            RES_LIMBS_PERMUTED.start,
            FIX_RANGE_CHECK_U8_PERMUTED.start + 8,
        );
        eval_lookups(
            vars,
            yield_constr,
            RES_LIMBS_PERMUTED.start + 1,
            FIX_RANGE_CHECK_U8_PERMUTED.start + 9,
        );
        eval_lookups(
            vars,
            yield_constr,
            RES_LIMBS_PERMUTED.start + 2,
            FIX_RANGE_CHECK_U8_PERMUTED.start + 10,
        );
        eval_lookups(
            vars,
            yield_constr,
            RES_LIMBS_PERMUTED.start + 3,
            FIX_RANGE_CHECK_U8_PERMUTED.start + 11,
        );
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
    use plonky2::plonk::config::{GenericConfig, PoseidonGoldilocksConfig};
    use plonky2::util::timing::TimingTree;
    use starky::config::StarkConfig;
    use starky::prover::prove as prove_table;
    use starky::verifier::verify_stark_proof;

    use crate::bitwise::stark::BitwiseStark;
    use crate::generation::bitwise::generate_bitwise_trace;
    use crate::stark::utils::trace_to_poly_values;

    #[test]
    fn prove_xor() -> Result<()> {
        const D: usize = 2;
        type C = PoseidonGoldilocksConfig;
        type F = <C as GenericConfig<D>>::F;
        type S = BitwiseStark<F, D>;
        let mut config = StarkConfig::standard_fast_config();
        config.fri_config.cap_height = 0;

        let stark = S::default();
        let record = simple_test_code(
            &[Instruction {
                op: Op::XOR,
                args: Args {
                    rs1: 5,
                    rs2: 6,
                    rd: 7,
                    imm: 0,
                },
            }],
            &[],
            &[(5, 1), (6, 2)],
        );
        assert_eq!(record.last_state.get_register_value(7), 3);
        let trace = generate_bitwise_trace(&record.executed);
        let trace_poly_values = trace_to_poly_values(trace);

        let proof = prove_table::<F, C, S, D>(
            stark,
            &config,
            trace_poly_values,
            [],
            &mut TimingTree::default(),
        )?;
        verify_stark_proof(stark, proof, &config)
    }

    #[test]
    fn prove_xori() -> Result<()> {
        const D: usize = 2;
        type C = PoseidonGoldilocksConfig;
        type F = <C as GenericConfig<D>>::F;
        type S = BitwiseStark<F, D>;
        let mut config = StarkConfig::standard_fast_config();
        config.fri_config.cap_height = 0;

        let stark = S::default();
        let record = simple_test_code(
            &[Instruction {
                op: Op::XOR,
                args: Args {
                    rs1: 5,
                    rs2: 0,
                    rd: 7,
                    imm: 2,
                },
            }],
            &[],
            &[(5, 1)],
        );
        assert_eq!(record.last_state.get_register_value(7), 3);
        let trace = generate_bitwise_trace(&record.executed);
        let trace_poly_values = trace_to_poly_values(trace);

        let proof = prove_table::<F, C, S, D>(
            stark,
            &config,
            trace_poly_values,
            [],
            &mut TimingTree::default(),
        )?;
        verify_stark_proof(stark, proof, &config)
    }
}

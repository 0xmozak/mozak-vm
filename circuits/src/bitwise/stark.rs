use std::marker::PhantomData;

use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::hash::hash_types::RichField;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::plonk_common::reduce_with_powers;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};
use starky::stark::Stark;
use starky::vars::{StarkEvaluationTargets, StarkEvaluationVars};

use super::columns::{
    COMPRESS_LIMBS, COMPRESS_PERMUTED, FIX_COMPRESS_PERMUTED, FIX_RANGE_CHECK_U8_PERMUTED,
    NUM_BITWISE_COL, OP1, OP1_LIMBS, OP1_LIMBS_PERMUTED, OP2, OP2_LIMBS, OP2_LIMBS_PERMUTED, RES,
    RES_LIMBS, RES_LIMBS_PERMUTED,
};
use crate::lookup::eval_lookups;
use crate::utils::from_;

#[derive(Clone, Copy, Default)]
#[allow(clippy::module_name_repetitions)]
pub struct BitwiseStark<F, const D: usize> {
    pub compress_challenge: F,
    pub _f: PhantomData<F>,
}

impl<F: RichField, const D: usize> BitwiseStark<F, D> {
    pub fn new(compress_challenge: F) -> Self {
        Self {
            compress_challenge,
            ..Self::default()
        }
    }
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
        for (opx, opx_limbs) in [(OP1, OP1_LIMBS), (OP2, OP2_LIMBS), (RES, RES_LIMBS)] {
            let opx_limbs = lv[opx_limbs].to_vec();
            let computed_sum = reduce_with_powers(&opx_limbs, from_(1_u128 << 8));
            yield_constr.constraint(computed_sum - lv[opx]);
        }

        // Constrain compress logic.
        let beta = FE::from_basefield(self.compress_challenge);
        for i in 0..4 {
            yield_constr.constraint(
                lv[OP1_LIMBS.start + i]
                    + lv[OP2_LIMBS.start + i] * beta
                    + lv[RES_LIMBS.start + i] * beta * beta
                    - lv[COMPRESS_LIMBS.start + i],
            );
        }

        for (fix_range_check_u8_permuted, opx_limbs_permuted) in FIX_RANGE_CHECK_U8_PERMUTED.zip(
            OP1_LIMBS_PERMUTED
                .chain(OP2_LIMBS_PERMUTED)
                .chain(RES_LIMBS_PERMUTED),
        ) {
            eval_lookups(
                vars,
                yield_constr,
                opx_limbs_permuted,
                fix_range_check_u8_permuted,
            );
        }

        for (fix_compress_permuted, compress_permuted) in
            FIX_COMPRESS_PERMUTED.zip(COMPRESS_PERMUTED)
        {
            eval_lookups(vars, yield_constr, compress_permuted, fix_compress_permuted);
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
    use mozak_vm::instruction::{Args, Instruction, Op};
    use mozak_vm::test_utils::simple_test_code;
    use plonky2::util::timing::TimingTree;
    use starky::prover::prove as prove_table;
    use starky::verifier::verify_stark_proof;

    use crate::bitwise::stark::BitwiseStark;
    use crate::generation::bitwise::generate_bitwise_trace;
    use crate::stark::utils::trace_to_poly_values;
    use crate::test_utils::{standard_faster_config, C, D, F};

    type S = BitwiseStark<F, D>;

    fn simple_and_test(a: u32, b: u32, imm: u32) {
        let config = standard_faster_config();

        let record = simple_test_code(
            &[Instruction {
                op: Op::AND,
                args: Args {
                    rs1: 5,
                    rs2: 6,
                    rd: 7,
                    imm,
                },
            }],
            &[],
            &[(5, a), (6, b)],
        );
        assert_eq!(record.last_state.get_register_value(7), a & (b + imm));
        let (trace, beta) = generate_bitwise_trace(&record.executed);
        let trace_poly_values = trace_to_poly_values(trace);
        let stark = S::new(beta);

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
            fn prove_andi_proptest(a in any::<u32>(), b in any::<u32>()) {
                simple_and_test(a, 0, b);
            }
            #[test]
            fn prove_and_proptest(a in any::<u32>(), b in any::<u32>()) {
                simple_and_test(a, b, 0);
            }
    }
}

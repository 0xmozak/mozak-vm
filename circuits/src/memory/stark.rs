use std::marker::PhantomData;

use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::hash::hash_types::RichField;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};
use starky::stark::Stark;
use starky::vars::{StarkEvaluationTargets, StarkEvaluationVars};

use crate::memory::columns::{
    COL_MEM_ADDR, COL_MEM_CLK, COL_MEM_DIFF_ADDR, COL_MEM_DIFF_ADDR_INV, COL_MEM_DIFF_CLK,
    COL_MEM_OP, COL_MEM_PADDING, COL_MEM_VALUE, NUM_MEM_COLS,
};
use crate::memory::trace::{OPCODE_LB, OPCODE_SB};

#[derive(Copy, Clone, Default)]
#[allow(clippy::module_name_repetitions)]
pub struct MemoryStark<F, const D: usize> {
    pub _f: PhantomData<F>,
}

#[deny(clippy::missing_panics_doc)]
impl<F: RichField + Extendable<D>, const D: usize> Stark<F, D> for MemoryStark<F, D> {
    const COLUMNS: usize = NUM_MEM_COLS;
    const PUBLIC_INPUTS: usize = 0;

    // Constraints design: https://docs.google.com/presentation/d/1G4tmGl8V1W0Wqxv-MwjGjaM3zUF99dzTvFhpiood4x4/edit?usp=sharing
    fn eval_packed_generic<FE, P, const D2: usize>(
        &self,
        vars: StarkEvaluationVars<FE, P, { Self::COLUMNS }, { Self::PUBLIC_INPUTS }>,
        yield_constr: &mut ConstraintConsumer<P>,
    ) where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>, {
        let lv = vars.local_values;
        let nv = vars.next_values;

        let local_new_addr = lv[COL_MEM_DIFF_ADDR] * lv[COL_MEM_DIFF_ADDR_INV];
        let next_new_addr = nv[COL_MEM_DIFF_ADDR] * nv[COL_MEM_DIFF_ADDR_INV];
        yield_constr.constraint_first_row(lv[COL_MEM_OP] - FE::from_canonical_usize(OPCODE_SB));
        yield_constr.constraint_first_row(lv[COL_MEM_DIFF_ADDR] - lv[COL_MEM_ADDR]);
        yield_constr.constraint_first_row(local_new_addr - P::ONES);
        yield_constr.constraint_first_row(lv[COL_MEM_DIFF_CLK]);

        // lv[COL_MEM_PADDING] is {0, 1}
        yield_constr.constraint(lv[COL_MEM_PADDING] * (lv[COL_MEM_PADDING] - P::ONES));

        // lv[COL_MEM_OP] in {0, 1}
        yield_constr.constraint(lv[COL_MEM_OP] * (lv[COL_MEM_OP] - P::ONES));

        // a) if new_addr: op === sb
        yield_constr
            .constraint(local_new_addr * (lv[COL_MEM_OP] - FE::from_canonical_usize(OPCODE_SB)));

        // b) if not new_addr: diff_clk_next <== clk_next - clk_cur
        yield_constr.constraint_transition(
            (nv[COL_MEM_DIFF_CLK] - nv[COL_MEM_CLK] + lv[COL_MEM_CLK]) * (next_new_addr - P::ONES),
        );

        // c) if new_addr: diff_clk === 0
        yield_constr.constraint(local_new_addr * lv[COL_MEM_DIFF_CLK]);

        // d) diff_addr_next <== addr_next - addr_cur
        yield_constr
            .constraint_transition(nv[COL_MEM_DIFF_ADDR] - nv[COL_MEM_ADDR] + lv[COL_MEM_ADDR]);

        // e) if op_next == lb: value_next === value_cur
        yield_constr.constraint(
            (nv[COL_MEM_VALUE] - lv[COL_MEM_VALUE])
                * (P::ONES - nv[COL_MEM_OP] + FE::from_canonical_usize(OPCODE_LB)),
        );

        // f) (new_addr - 1)*diff_addr===0
        //    (new_addr - 1)*diff_addr_inv===0
        yield_constr.constraint((local_new_addr - P::ONES) * lv[COL_MEM_DIFF_ADDR]);
        yield_constr.constraint((local_new_addr - P::ONES) * lv[COL_MEM_DIFF_ADDR_INV]);
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
    use plonky2::plonk::config::{GenericConfig, PoseidonGoldilocksConfig};
    use plonky2::util::timing::TimingTree;
    use starky::config::StarkConfig;
    use starky::prover::prove as prove_table;
    use starky::stark_testing::test_stark_low_degree;
    use starky::verifier::verify_stark_proof;

    use crate::generation::memory::generate_memory_trace;
    use crate::memory::stark::MemoryStark;
    use crate::memory::test_utils::memory_trace_test_case;
    use crate::stark::utils::trace_to_poly_values;

    const D: usize = 2;
    type C = PoseidonGoldilocksConfig;
    type F = <C as GenericConfig<D>>::F;
    type S = MemoryStark<F, D>;

    #[test]
    fn test_degree() -> Result<()> {
        let stark = S::default();
        test_stark_low_degree(stark)
    }

    #[test]
    fn prove_memory_sb_lb() -> Result<()> {
        let mut config = StarkConfig::standard_fast_config();
        config.fri_config.cap_height = 0;

        let stark = S::default();
        let executed = memory_trace_test_case();
        let trace = generate_memory_trace(executed);
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

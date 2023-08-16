use std::borrow::Borrow;
use std::marker::PhantomData;

use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::hash::hash_types::RichField;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};
use starky::stark::Stark;
use starky::vars::{StarkEvaluationTargets, StarkEvaluationVars};

use crate::cpu::stark::is_binary;
use crate::memory::columns::{Memory, NUM_MEM_COLS};
use crate::memory::trace::OPCODE_SB;

#[derive(Copy, Clone, Default)]
#[allow(clippy::module_name_repetitions)]
pub struct MemoryStark<F, const D: usize> {
    pub _f: PhantomData<F>,
}

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
        let lv: &Memory<P> = vars.local_values.borrow();
        let nv: &Memory<P> = vars.next_values.borrow();

        let diff_addr = nv.addr - lv.addr;
        // This still forbids 0 as the first address.
        // That's wrong.
        let local_new_addr = lv.diff_addr * lv.diff_addr_inv;
        let next_new_addr = diff_addr * nv.diff_addr_inv;
        yield_constr.constraint_first_row(lv.op - FE::from_canonical_usize(OPCODE_SB));
        yield_constr.constraint_first_row(lv.diff_addr - lv.addr);
        yield_constr.constraint_first_row(lv.diff_clk);

        is_binary(yield_constr, lv.is_executed);

        // Once we have padding, all subsequent rows are padding; ie not
        // `is_executed`.
        yield_constr.constraint_transition((lv.is_executed - nv.is_executed) * nv.is_executed);

        // We only have two different ops at the moment, so we use a binary variable to
        // represent them:
        is_binary(yield_constr, lv.op);

        // a) if new_addr: op === sb
        yield_constr.constraint(local_new_addr * (lv.op - FE::from_canonical_usize(OPCODE_SB)));

        // b) if not new_addr: diff_clk_next <== clk_next - clk_cur
        yield_constr
            .constraint_transition((nv.diff_clk - nv.clk + lv.clk) * (next_new_addr - P::ONES));

        // c) if new_addr: diff_clk === 0
        yield_constr.constraint(local_new_addr * lv.diff_clk);

        // d) diff_addr_next <== addr_next - addr_cur
        yield_constr.constraint_transition(nv.diff_addr - diff_addr);

        // e) if op_next != sb: value_next === value_cur
        yield_constr
            .constraint((nv.op - FE::from_canonical_usize(OPCODE_SB)) * (nv.value - lv.value));

        // f) (new_addr - 1)*diff_addr===0
        //    (new_addr - 1)*diff_addr_inv===0
        yield_constr.constraint((local_new_addr - P::ONES) * lv.diff_addr);
        yield_constr.constraint((local_new_addr - P::ONES) * lv.diff_addr_inv);
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
    use starky::stark_testing::test_stark_low_degree;

    use crate::memory::stark::MemoryStark;
    use crate::memory::test_utils::memory_trace_test_case;
    use crate::stark::mozak_stark::MozakStark;
    use crate::test_utils::ProveAndVerify;

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
        for repeats in 0..8 {
            let (program, executed) = memory_trace_test_case(repeats);
            MozakStark::prove_and_verify(&program, &executed)?;
        }
        Ok(())
    }
}

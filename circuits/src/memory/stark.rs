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
        // Both `new_addr` values are 1 if the address changed, 0 otherwise
        // This is like a normalised diff_addr.
        let local_new_addr = lv.diff_addr * lv.diff_addr_inv;
        let next_new_addr = diff_addr * nv.diff_addr_inv;
        // For the initial state of memory access, we request:
        // 1. First opcode is `sb`
        // 2. `diff_addr` is initiated as `addr - 0`
        // M: TODO(Matthias): Allow writing to address 0.
        // 3. `addr` != 0
        // 4. `diff_clk` is initiated as `0`
        yield_constr.constraint_first_row(lv.op - FE::from_canonical_usize(OPCODE_SB));
        // yield_constr.constraint_first_row(lv.diff_addr - lv.addr);
        yield_constr.constraint_first_row(lv.diff_clk);

        // Consequently, we constrain:

        is_binary(yield_constr, lv.is_executed);

        // Once we have padding, all subsequent rows are padding; ie not
        // `is_executed`.
        yield_constr.constraint_transition((lv.is_executed - nv.is_executed) * nv.is_executed);

        // We only have two different ops at the moment, so we use a binary variable to
        // represent them:
        is_binary(yield_constr, lv.op);

        // Check: if address for next instruction changed, then opcode was `sb`
        yield_constr.constraint(local_new_addr * (lv.op - FE::from_canonical_usize(OPCODE_SB)));

        // M: OK, this would be removed by remove the diff column?
        // M: Tough: how do we build the filter for the ctl?  Probably via _is_init_ column?
        // Check: if next address did not change, diff_clk_next is `clk` difference
        yield_constr
            .constraint_transition((nv.diff_clk - nv.clk + lv.clk) * (next_new_addr - P::ONES));

        // Check: if address changed, then clock did not change
        // M: OK, we arbitrarily set the clock diff to 0.  That's not necessary, or is it?
        // yield_constr.constraint(local_new_addr * lv.diff_clk);
        // yield_constr.constraint(lv.diff_addr * lv.diff_clk);

        // Check: `diff_addr_next` is  `addr_next - addr_cur`
        yield_constr.constraint_transition(nv.diff_addr - diff_addr);

        // Check: either the next operation is a store or the `value` stays the same.
        yield_constr
            .constraint((nv.op - FE::from_canonical_usize(OPCODE_SB)) * (nv.value - lv.value));

        // Check: either `diff_addr_inv` is inverse of `diff_addr`, or they both are 0.
        yield_constr.constraint((local_new_addr - P::ONES) * lv.diff_addr);
        yield_constr.constraint((local_new_addr - P::ONES) * lv.diff_addr_inv);

        // Once we have padding, all subsequent rows are padding; ie not
        // `is_executed`.
        yield_constr.constraint_transition((lv.is_executed - nv.is_executed) * nv.is_executed);
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
    fn prove_memory_sb_lb_all() -> Result<()> {
        let (program, executed) = memory_trace_test_case(1);
        MozakStark::prove_and_verify(&program, &executed)?;
        Ok(())
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

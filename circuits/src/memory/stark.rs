use std::borrow::Borrow;
use std::marker::PhantomData;
use std::ops::Add;

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
use crate::stark::utils::{are_equal, is_not};

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

        // Boolean variables describing whether the current row and
        // next row has a change of address when compared to the
        // previous entry in the table. This works on the assumption
        // that any change in addr will have non-zero `diff_addr` and
        // consequently `diff_addr_inv` values. Any non-zero values for
        // `diff_addr` will lead "correct" `diff_addr_inv` to be multiplicative
        // inverse in the field leading multiplied value `1`. In case there is
        // no change in addr, `diff_addr` (and consequently `diff_addr_inv`)
        // remain `0` when multiplied to each other give `0`.
        let (is_local_a_new_addr, is_next_a_new_addr) = (
            lv.diff_addr * lv.diff_addr_inv,    // constrained below
            nv.diff_addr * nv.diff_addr_inv,    // constrained below
        );

        // Boolean constraints
        // -------------------
        // Constrain certain columns of the memory table to be only
        // exercising boolean values.
        is_binary(yield_constr, lv.is_executed);
        is_binary(yield_constr, lv.is_writable);
        is_binary(yield_constr, lv.is_init);
        // Also ensure that "difference" values are consistent with their inverses
        is_binary(yield_constr, is_local_a_new_addr);
        is_binary(yield_constr, is_next_a_new_addr);

        // First row constraints
        // ---------------------
        // When starting off, the first `addr` we encounter is supposed to be
        // relatively away from `0` by `diff_addr`, consequently `addr` and
        // `diff_addr` are same for the first row. As a matter of preference,
        // we can have any `clk` in the first row, but `diff_clk` is `0`.
        // This is because when `addr` changes, `diff_clk` is expected to be `0`.
        yield_constr.constraint_first_row(lv.diff_addr - lv.addr);
        yield_constr.constraint_first_row(lv.diff_clk);

        // Ascending ordered, contigous "address" view constraint
        // ------------------------------------------------------
        // All memory init / accesses for a given `addr` is described via contigous
        // rows. This is constrained by range-check on `diff_addr` which in 32-bit
        // RISC can only assume values 0 till 2^32-1. If similar range-checking
        // constraint is put on `addr` as well, the only possibility of
        // non-contigous address view occurs when the prime order of field in
        // question is of size less than 2*(2^32 - 1). Both `.addr` and `.diff_addr`
        // is constrained in `pub fn rangecheck_looking<F: Field>()` subsequently.

        // Memory initialization Constraints
        // ---------------------------------
        // Memory table is assumed to be ordered by `addr` in asc order.
        // such that whenever we describe an memory init / access
        // pattern of an "address", a correct table gurantees the following:
        //    All rows for a specific `addr` start with either a memory init (via static
        //    ELF) with `is_init` flag set (case for ro or rw static memory) or `SB`
        //    (case for heap / other dynamic addresses). It is assumed that static
        //    memory init operation happens before any execution has started and
        //    consequently `clk` should be `0` for such entries.
        // NOTE: We rely on 'Ascending ordered, contigous "address" view constraint'
        // since if that is broken, for same address different contigous blocks could
        // present case for being derived from static ELF and dynamic (execution) at
        // the same time.

        // Ensure all `is_init` entries are only when `is_executed` is `1`.
        // If `is_init` == `1` and `is_executed` == `0`, the following is
        // not binary.
        is_binary(yield_constr, lv.is_executed - lv.is_init);

        // If the `addr` talks about an address coming from a static address-space
        // i.e. ELF itself, it has first row as `is_init` and the `clk` would be `0`.
        yield_constr.constraint(
            is_local_a_new_addr * lv.is_init    // selector
            * lv.clk, // constrain clk to be `0` if selector == true
        );

        // If instead, the `addr` talks about an address not coming from static ELF,
        // it needs to begin with a `SB` (store) operation before any further access
        yield_constr.constraint(
            is_local_a_new_addr * is_not(lv.is_init)                            // selector
            * are_equal(lv.op, FE::from_canonical_usize(OPCODE_SB).into()) // constrain `SB` as operation if selector == true
        );

        // However, `SB` based initialization can not occur on read-only marked memory
        // We are assuming no other store operations exist (half word or full word)
        yield_constr.constraint(
            is_local_a_new_addr * is_not(lv.is_writable)
            * is_not(are_equal(lv.op, FE::from_canonical_usize(OPCODE_SB).into()))
        );

        // Operation constraints
        // ---------------------
        // Currently we only support `SB` and `LB` operations (no half-word or full-word
        // load and store). These are represented in `op` as either `0` or `1`. We
        // constrain them here
        is_binary(yield_constr, lv.op);

        // Clock constraints
        // -----------------
        // `diff_clk` assumes the value "new row's `clk`" - "current row's `clk`" in
        // case both new row and current row talk about the same addr. However,
        // in case the "new row" describes an `addr` different from the current
        // row, we expect `diff_clk` to be `0`. New row's clk remains
        // unconstrained in such situation.
        yield_constr.constraint_transition(
            is_not(is_next_a_new_addr)              // selector
            * are_equal(nv.diff_clk, nv.clk - lv.clk), /* `diff_clk` matches difference if
                                                        * selector == true */
        );
        yield_constr.constraint_transition(
            is_local_a_new_addr         // selector
            * lv.diff_clk, // `diff_clk` is `0` in case a selector == true
        );

        // Address constraints
        // -------------------
        // We need to ensure that `diff_addr` always encapsulates difference in addr
        // between two rows
        yield_constr.constraint_transition(are_equal(nv.addr, lv.addr + nv.diff_addr));

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
            MemoryStark::prove_and_verify(&program, &executed)?;
        }
        Ok(())
    }
}

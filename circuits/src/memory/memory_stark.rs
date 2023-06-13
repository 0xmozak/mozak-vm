use std::marker::PhantomData;

use mozak_vm::instruction::Op;
use plonky2::field::extension::FieldExtension;
use plonky2::field::packed::PackedField;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::{field::extension::Extendable, hash::hash_types::RichField};
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};
use starky::stark::Stark;
use starky::vars::{StarkEvaluationTargets, StarkEvaluationVars};

use super::{columns::*, *};
use crate::memory::trace::OPCODE_SB;
use crate::stark::utils::opcode_one_hot;
use crate::utils::from_;

#[derive(Copy, Clone, Default)]
pub struct MemoryStark<F, const D: usize> {
    pub _f: PhantomData<F>,
}

impl<F: RichField + Extendable<D>, const D: usize> Stark<F, D> for MemoryStark<F, D> {
    const COLUMNS: usize = NUM_MEM_COLS;
    const PUBLIC_INPUTS: usize = 0;

    // Constraints:
    //
    // lv.padding is {0, 1}
    // lv.op in {SB, LB}
    //
    // If nv.addr != lv.addr:
    //   nv.op === SB
    //   nv.diff_addr <== nv.addr - lv.addr
    //   TODO: (range check) nv.diff_addr is a u32
    //
    // If nv.addr == lv.addr:
    //   nv.diff_addr === 0
    //
    // If nv.op == LB:
    //   nv.addr === lv.addr
    //   nv.value === lv.value
    //   nv.diff_clk <== nv.clk - lv.clk
    //   TODO: (range check) nv.diff_clk < total time?
    fn eval_packed_generic<FE, P, const D2: usize>(
        &self,
        vars: StarkEvaluationVars<FE, P, { Self::COLUMNS }, { Self::PUBLIC_INPUTS }>,
        yield_constr: &mut ConstraintConsumer<P>,
    ) where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>,
    {
        let lv = vars.local_values;
        let nv = vars.next_values;

        // lv[COL_MEM_PADDING] is {0, 1}
        yield_constr.constraint(lv[COL_MEM_PADDING] * (lv[COL_MEM_PADDING] - P::ONES));

        // lv[COL_MEM_OP] in {0, 1}
        yield_constr.constraint(lv[COL_MEM_OP] * (lv[COL_MEM_OP] - P::ONES));

        // nv[COL_MEM_DIFF_ADDR] = nv[COL_MEM_ADDR] - lv[COL_MEM_ADDR]
        yield_constr.constraint(nv[COL_MEM_DIFF_ADDR] - nv[COL_MEM_ADDR] + lv[COL_MEM_ADDR]);

        // When address changed, clk difference should be zero.
        yield_constr.constraint(lv[COL_MEM_NEW_ADDR] * lv[COL_MEM_DIFF_CLK]);

        // When address changed, nv[COL_MEM_OP] should be SB
        yield_constr.constraint(
            lv[COL_MEM_NEW_ADDR] * (lv[COL_MEM_OP] - F::from_canonical_usize(OPCODE_SB)),
        );

        // When address not changed, nv[COL_MEM_DIFF_CLK] = nv[COL_MEM_CLK] -
        // lv[COL_MEM_CLK]
        yield_constr.constraint(
            (nv[COL_MEM_DIFF_CLK] - nv[COL_MEM_CLK] + lv[COL_MEM_CLK])
                * (nv[COL_MEM_NEW_ADDR] - P::ONES),
        );
    }

    fn constraint_degree(&self) -> usize {
        2
    }

    fn eval_ext_circuit(
        &self,
        _builder: &mut CircuitBuilder<F, D>,
        _vars: StarkEvaluationTargets<D, { Self::COLUMNS }, { Self::PUBLIC_INPUTS }>,
        _yield_constr: &mut RecursiveConstraintConsumer<F, D>,
    ) {
        unimplemented!()
    }
}

use std::marker::PhantomData;

use plonky2::field::extension::FieldExtension;
use plonky2::field::packed::PackedField;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::{field::extension::Extendable, hash::hash_types::RichField};
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
        P: PackedField<Scalar = FE>,
    {
        let lv = vars.local_values;
        let nv = vars.next_values;

        let new_address = lv[COL_MEM_DIFF_ADDR] * lv[COL_MEM_DIFF_ADDR_INV];
        yield_constr.constraint_first_row(lv[COL_MEM_OP] - FE::from_canonical_usize(OPCODE_SB));
        yield_constr.constraint_first_row(new_address);
        yield_constr.constraint_first_row(lv[COL_MEM_DIFF_ADDR]);
        yield_constr.constraint_first_row(lv[COL_MEM_DIFF_CLK]);

        // lv[COL_MEM_PADDING] is {0, 1}
        yield_constr.constraint(lv[COL_MEM_PADDING] * (lv[COL_MEM_PADDING] - P::ONES));

        // lv[COL_MEM_OP] in {0, 1}
        yield_constr.constraint(lv[COL_MEM_OP] * (lv[COL_MEM_OP] - P::ONES));

        // a1) When address changed, nv[COL_MEM_OP] should be SB
        yield_constr
            .constraint(new_address * (lv[COL_MEM_OP] - FE::from_canonical_usize(OPCODE_SB)));
        // a2) If nv[COL_MEM_OP] == LB, nv[COL_MEM_ADDR] == lv[COL_MEM_ADDR]
        yield_constr.constraint(
            new_address * (P::ONES - nv[COL_MEM_OP] + FE::from_canonical_usize(OPCODE_LB)),
        );

        // b) When address not changed, nv[COL_MEM_DIFF_CLK] = nv[COL_MEM_CLK] -
        //    lv[COL_MEM_CLK]
        yield_constr.constraint(
            (nv[COL_MEM_DIFF_CLK] - nv[COL_MEM_CLK] + lv[COL_MEM_CLK]) * (new_address - P::ONES),
        );

        // c) nv[COL_MEM_DIFF_ADDR] = nv[COL_MEM_ADDR] - lv[COL_MEM_ADDR]
        yield_constr.constraint(nv[COL_MEM_DIFF_ADDR] - nv[COL_MEM_ADDR] + lv[COL_MEM_ADDR]);

        // d) When address changed, clk difference should be zero.
        yield_constr.constraint(new_address * lv[COL_MEM_DIFF_CLK]);

        // e) If nv[COL_MEM_OP] == LB, nv[COL_MEM_VALUE] == lv[COL_MEM_VALUE]
        yield_constr.constraint(
            (nv[COL_MEM_VALUE] - lv[COL_MEM_VALUE])
                * (P::ONES - nv[COL_MEM_OP] + FE::from_canonical_usize(OPCODE_LB)),
        );

        // f) When address not changed, lv[COL_MEM_DIFF_ADDR] = 0
        yield_constr.constraint(lv[COL_MEM_DIFF_ADDR] * (new_address - P::ONES));
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

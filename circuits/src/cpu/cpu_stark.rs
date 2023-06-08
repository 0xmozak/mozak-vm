use std::marker::PhantomData;

use anyhow::Result;
use plonky2::field::extension::FieldExtension;
use plonky2::field::packed::PackedField;
use plonky2::{field::extension::Extendable, hash::hash_types::RichField};

use super::columns::*;
use starky::constraint_consumer::ConstraintConsumer;
use crate::stark::stark::Stark;
use crate::stark::vars::StarkEvaluationVars;

#[derive(Copy, Clone, Default)]
pub struct CpuStark<F, const D: usize> {
    compress_challenge: Option<F>,
    pub f: PhantomData<F>,
}

impl<F: RichField, const D: usize> CpuStark<F, D> {
    pub fn set_compress_challenge(&mut self, challenge: F) -> Result<()> {
        assert!(self.compress_challenge.is_none(), "already set?");
        self.compress_challenge = Some(challenge);
        Ok(())
    }
    pub fn get_compress_challenge(&self) -> Option<F> {
        self.compress_challenge
    }
}

impl<F: RichField + Extendable<D>, const D: usize> Stark<F, D> for CpuStark<F, D> {
    const COLUMNS: usize = NUM_CPU_COLS;
    fn eval_packed_generic<FE, P, const D2: usize>(
        &self,
        vars: StarkEvaluationVars<FE, P, NUM_CPU_COLS>,
        yield_constr: &mut ConstraintConsumer<P>,
    ) where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>,
    {
        let lv = vars.local_values;
        let nv = vars.next_values;
        // Selector of opcode, builtins and halt should be binary.
        let op_selectors = [lv[COL_S_ADD], lv[COL_S_HALT]];

        op_selectors
            .iter()
            .for_each(|s| yield_constr.constraint(*s * (P::ONES - *s)));

        // Only one opcode selector enabled.
        let sum_s_op: P = op_selectors.into_iter().sum();
        yield_constr.constraint(P::ONES - sum_s_op);

        // Constrain state changing.
        // clk
        // if its already halted we can ignore this constraint
        yield_constr
            .constraint((P::ONES - lv[COL_S_HALT]) * (nv[COL_CLK] - (lv[COL_CLK] + P::ONES)));

        // Registers
        // Register used as destination register can had different value, all other regs
        // have same value as of previous row.
        for reg in 0..32 {
            let reg_index = COL_REGS.start + reg;
            yield_constr.constraint(
                (lv[COL_RD] - P::Scalar::from_canonical_u32(reg as u32))
                    * (lv[reg_index] - nv[reg_index]),
            );
        }

        // pc
        // if its already halted we can ignore this constraint
        let incr_wo_branch = P::ONES + P::ONES + P::ONES + P::ONES;
        let pc_incr = lv[COL_PC] + incr_wo_branch;
        yield_constr.constraint((P::ONES - lv[COL_S_HALT]) * (nv[COL_PC] - pc_incr));

        // add constraint

        // Last row must be HALT
        yield_constr.constraint_last_row(lv[COL_S_HALT] - P::ONES);
    }

    fn constraint_degree(&self) -> usize {
        5
    }
}

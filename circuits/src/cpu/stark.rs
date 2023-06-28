use std::marker::PhantomData;

use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::hash::hash_types::RichField;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};
use starky::stark::Stark;
use starky::vars::{StarkEvaluationTargets, StarkEvaluationVars};

use super::columns::{
    COL_CLK, COL_PC, COL_RD, COL_REGS, COL_S_ADD, COL_S_BEQ, COL_S_ECALL, COL_S_HALT, COL_S_SUB,
    NUM_CPU_COLS,
};
use super::{add, sub};
use crate::utils::{column_of_xs, from_};

#[derive(Copy, Clone, Default)]
#[allow(clippy::module_name_repetitions)]
pub struct CpuStark<F, const D: usize> {
    pub _f: PhantomData<F>,
}

use array_concat::{concat_arrays, concat_arrays_size};

pub const STRAIGHTLINE_OPCODES: [usize; 3] = [COL_S_ADD, COL_S_ECALL, COL_S_SUB];
pub const JUMPING_OPCODES: [usize; 1] = [COL_S_BEQ];
pub const OPCODES: [usize; concat_arrays_size!(STRAIGHTLINE_OPCODES, JUMPING_OPCODES)] =
    concat_arrays!(STRAIGHTLINE_OPCODES, JUMPING_OPCODES);

fn pc_ticks_up<P: PackedField>(
    lv: &[P; NUM_CPU_COLS],
    nv: &[P; NUM_CPU_COLS],
    yield_constr: &mut ConstraintConsumer<P>,
) {
    let is_straightline_op: P = STRAIGHTLINE_OPCODES
        .into_iter()
        .map(|op_code| lv[op_code])
        .sum();
    yield_constr
        .constraint_transition(is_straightline_op * (nv[COL_PC] - (lv[COL_PC] + column_of_xs::<P>(4))));
}

/// Selector of opcode, builtins and halt should be one-hot encoded.
///
/// Ie exactly one of them should be by 1, all others by 0 in each row.
/// See <https://en.wikipedia.org/wiki/One-hot>
fn opcode_one_hot<P: PackedField>(
    lv: &[P; NUM_CPU_COLS],
    yield_constr: &mut ConstraintConsumer<P>,
) {
    let op_selectors: Vec<_> = OPCODES.iter().map(|&op_code| lv[op_code]).collect();

    // Op selectors have value 0 or 1.
    op_selectors
        .iter()
        .for_each(|&s| yield_constr.constraint(s * (P::ONES - s)));

    // Only one opcode selector enabled.
    let sum_s_op: P = op_selectors.into_iter().sum();
    yield_constr.constraint(P::ONES - sum_s_op);
}

/// Ensure clock is ticking up
fn clock_ticks<P: PackedField>(
    lv: &[P; NUM_CPU_COLS],
    nv: &[P; NUM_CPU_COLS],
    yield_constr: &mut ConstraintConsumer<P>,
) {
    yield_constr.constraint_transition(nv[COL_CLK] - (lv[COL_CLK] + P::ONES));
}

/// Register used as destination register can have different value, all
/// other regs have same value as of previous row.
fn only_rd_changes<P: PackedField>(
    lv: &[P; NUM_CPU_COLS],
    nv: &[P; NUM_CPU_COLS],
    yield_constr: &mut ConstraintConsumer<P>,
) {
    // Note: register 0 is already always 0.
    // But we keep the constraints simple here.
    for reg in 0_u32..32 {
        let reg_index = COL_REGS.start + reg as usize;
        let x: P::Scalar = from_(reg);
        yield_constr.constraint_transition((lv[COL_RD] - x) * (lv[reg_index] - nv[reg_index]));
    }
}

/// Register 0 is always 0
fn r0_always_0<P: PackedField>(lv: &[P; NUM_CPU_COLS], yield_constr: &mut ConstraintConsumer<P>) {
    yield_constr.constraint(lv[COL_REGS.start]);
}

impl<F: RichField + Extendable<D>, const D: usize> Stark<F, D> for CpuStark<F, D> {
    const COLUMNS: usize = NUM_CPU_COLS;
    const PUBLIC_INPUTS: usize = 0;

    fn eval_packed_generic<FE, P, const D2: usize>(
        &self,
        vars: StarkEvaluationVars<FE, P, { Self::COLUMNS }, { Self::PUBLIC_INPUTS }>,
        yield_constr: &mut ConstraintConsumer<P>,
    ) where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>, {
        let lv = vars.local_values;
        let nv = vars.next_values;

        opcode_one_hot(lv, yield_constr);

        clock_ticks(lv, nv, yield_constr);
        pc_ticks_up(lv, nv, yield_constr);

        // Registers
        only_rd_changes(lv, nv, yield_constr);
        r0_always_0(lv, yield_constr);

        // add constraint
        add::constraints(lv, yield_constr);
        sub::constraints(lv, yield_constr);

        // Last row must be HALT
        yield_constr.constraint_last_row(lv[COL_S_HALT] - P::ONES);
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

use std::marker::PhantomData;

use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use plonky2::hash::hash_types::RichField;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};
use starky::stark::Stark;
use starky::vars::{StarkEvaluationTargets, StarkEvaluationVars};

use super::columns::{
    CLK, DST_VALUE, IMM_VALUE, NUM_CPU_COLS, OP1_VALUE, OP2_VALUE, PC, RD_SELECT, REGS, RS1_SELECT,
    RS2_SELECT, S_ADD, S_AND, S_BEQ, S_BNE, S_DIVU, S_ECALL, S_HALT, S_JALR, S_MUL, S_MULHU, S_OR,
    S_REMU, S_SLL, S_SLT, S_SLTU, S_SRL, S_SUB, S_XOR,
};
use super::{add, beq, bitwise, div, jalr, mul, slt, sub};

#[derive(Copy, Clone, Default)]
#[allow(clippy::module_name_repetitions)]
pub struct CpuStark<F, const D: usize> {
    pub _f: PhantomData<F>,
}

use array_concat::{concat_arrays, concat_arrays_size};

pub const STRAIGHTLINE_OPCODES: [usize; 13] = [
    S_ADD, S_SUB, S_AND, S_OR, S_XOR, S_DIVU, S_MUL, S_MULHU, S_REMU, S_SLL, S_SLT, S_SLTU, S_SRL,
];
pub const JUMPING_OPCODES: [usize; 4] = [S_BEQ, S_BNE, S_ECALL, S_JALR];
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
    yield_constr.constraint_transition(
        is_straightline_op * (nv[PC] - (lv[PC] + P::Scalar::from_noncanonical_u64(4))),
    );
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
    yield_constr.constraint_transition(nv[CLK] - (lv[CLK] + P::ONES));
}

/// Register 0 is always 0
fn r0_always_0<P: PackedField>(lv: &[P; NUM_CPU_COLS], yield_constr: &mut ConstraintConsumer<P>) {
    yield_constr.constraint(lv[REGS[0]]);
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
    (0..32).for_each(|reg| {
        yield_constr.constraint_transition(
            (P::ONES - lv[RD_SELECT[reg]]) * (lv[REGS[reg]] - nv[REGS[reg]]),
        );
    });
}

fn rd_actually_changes<P: PackedField>(
    lv: &[P; NUM_CPU_COLS],
    nv: &[P; NUM_CPU_COLS],
    yield_constr: &mut ConstraintConsumer<P>,
) {
    // Note: we skip 0 here, because it's already forced to 0 permanently by
    // `r0_always_0`
    (1..32).for_each(|reg| {
        yield_constr.constraint_transition((lv[RD_SELECT[reg]]) * (lv[DST_VALUE] - nv[REGS[reg]]));
    });
}

fn populate_op1_value<P: PackedField>(
    lv: &[P; NUM_CPU_COLS],
    yield_constr: &mut ConstraintConsumer<P>,
) {
    yield_constr.constraint(
        lv[OP1_VALUE]
            // Note: we could skip 0, because r0 is always 0.
            // But we keep the constraints simple here.
            - (0..32)
                .map(|reg| lv[RS1_SELECT[reg]] * lv[REGS[reg]])
                .sum::<P>(),
    );
}

fn populate_op2_value<P: PackedField>(
    lv: &[P; NUM_CPU_COLS],
    yield_constr: &mut ConstraintConsumer<P>,
) {
    yield_constr.constraint(
        lv[OP2_VALUE]
            - lv[IMM_VALUE]
            // Note: we could skip 0, because r0 is always 0.
            // But we keep the constraints simple here.
            - (0..32)
                .map(|reg| lv[RS2_SELECT[reg]] * lv[REGS[reg]])
                .sum::<P>(),
    );
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
        r0_always_0(lv, yield_constr);
        only_rd_changes(lv, nv, yield_constr);
        rd_actually_changes(lv, nv, yield_constr);
        populate_op1_value(lv, yield_constr);
        populate_op2_value(lv, yield_constr);

        // add constraint
        add::constraints(lv, yield_constr);
        sub::constraints(lv, yield_constr);
        bitwise::constraints(lv, yield_constr);
        slt::constraints(lv, yield_constr);
        beq::constraints(lv, nv, yield_constr);
        div::constraints(lv, yield_constr);
        mul::constraints(lv, yield_constr);
        jalr::constraints(lv, nv, yield_constr);

        // Last row must be HALT
        yield_constr.constraint_last_row(lv[S_HALT] - P::ONES);
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

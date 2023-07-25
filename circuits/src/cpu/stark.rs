use std::borrow::Borrow;
use std::marker::PhantomData;

use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use plonky2::hash::hash_types::RichField;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};
use starky::permutation::PermutationPair;
use starky::stark::Stark;
use starky::vars::{StarkEvaluationTargets, StarkEvaluationVars};

use super::columns::{CpuColumnsView, OpSelectorView, MAP};
use super::{add, beq, bitwise, div, jalr, mul, shift_amount, slt, sub};
use crate::columns_view::NumberOfColumns;

#[derive(Copy, Clone, Default)]
#[allow(clippy::module_name_repetitions)]
pub struct CpuStark<F, const D: usize> {
    pub _f: PhantomData<F>,
}

impl<P: Copy> OpSelectorView<P> {
    fn straightline_opcodes(&self) -> Vec<P> {
        vec![
            self.add, self.sub, self.and, self.or, self.xor, self.divu, self.mul, self.mulhu,
            self.remu, self.sll, self.slt, self.sltu, self.srl,
        ]
    }

    fn jumping_opcodes(&self) -> Vec<P> { vec![self.beq, self.bne, self.ecall, self.jalr] }

    fn opcodes(&self) -> Vec<P> {
        let mut res = self.straightline_opcodes();
        res.extend(self.jumping_opcodes());
        res
    }
}

fn pc_ticks_up<P: PackedField>(
    lv: &CpuColumnsView<P>,
    nv: &CpuColumnsView<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    let is_straightline_op: P = lv.ops.straightline_opcodes().into_iter().sum();

    yield_constr.constraint_transition(
        is_straightline_op * (nv.pc - (lv.pc + P::Scalar::from_noncanonical_u64(4))),
    );
}

/// Selector of opcode, builtins and halt should be one-hot encoded.
///
/// Ie exactly one of them should be by 1, all others by 0 in each row.
/// See <https://en.wikipedia.org/wiki/One-hot>
fn opcode_one_hot<P: PackedField>(
    lv: &CpuColumnsView<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    let op_selectors: Vec<_> = lv.ops.opcodes();

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
    lv: &CpuColumnsView<P>,
    nv: &CpuColumnsView<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    yield_constr.constraint_transition(nv.clk - (lv.clk + P::ONES));
}

/// Register 0 is always 0
fn r0_always_0<P: PackedField>(lv: &CpuColumnsView<P>, yield_constr: &mut ConstraintConsumer<P>) {
    yield_constr.constraint(lv.regs[0]);
}

/// Register used as destination register can have different value, all
/// other regs have same value as of previous row.
fn only_rd_changes<P: PackedField>(
    lv: &CpuColumnsView<P>,
    nv: &CpuColumnsView<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    // Note: register 0 is already always 0.
    // But we keep the constraints simple here.
    (0..32).for_each(|reg| {
        yield_constr
            .constraint_transition((P::ONES - lv.rd_select[reg]) * (lv.regs[reg] - nv.regs[reg]));
    });
}

fn rd_actually_changes<P: PackedField>(
    lv: &CpuColumnsView<P>,
    nv: &CpuColumnsView<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    // Note: we skip 0 here, because it's already forced to 0 permanently by
    // `r0_always_0`
    (1..32).for_each(|reg| {
        yield_constr.constraint_transition((lv.rd_select[reg]) * (lv.dst_value - nv.regs[reg]));
    });
}

fn populate_op1_value<P: PackedField>(
    lv: &CpuColumnsView<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    yield_constr.constraint(
        lv.op1_value
            // Note: we could skip 0, because r0 is always 0.
            // But we keep the constraints simple here.
            - (0..32)
                .map(|reg| lv.rs1_select[reg] * lv.regs[reg])
                .sum::<P>(),
    );
}

/// `OP2_VALUE` is the sum of the value of the second operand register and the
/// immediate value.
fn populate_op2_value<P: PackedField>(
    lv: &CpuColumnsView<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    yield_constr.constraint(
        lv.op2_value - lv.imm_value
            // Note: we could skip 0, because r0 is always 0.
            // But we keep the constraints simple here.
            - (0..32)
                .map(|reg| lv.rs2_select[reg] * lv.regs[reg])
                .sum::<P>(),
    );
}

impl<F: RichField + Extendable<D>, const D: usize> Stark<F, D> for CpuStark<F, D> {
    const COLUMNS: usize = CpuColumnsView::<F>::NUMBER_OF_COLUMNS;
    const PUBLIC_INPUTS: usize = 0;

    fn eval_packed_generic<FE, P, const D2: usize>(
        &self,
        vars: StarkEvaluationVars<FE, P, { Self::COLUMNS }, { Self::PUBLIC_INPUTS }>,
        yield_constr: &mut ConstraintConsumer<P>,
    ) where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>, {
        let lv = vars.local_values.borrow();
        let nv = vars.next_values.borrow();
        // let lv: &BitwiseColumnsView<_> = vars.local_values.borrow();

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
        shift_amount::constraints_on_shamt(vars, yield_constr);
        shift_amount::constraints_on_power_of_2_shamt(vars, yield_constr);

        // Last row must be HALT
        yield_constr.constraint_last_row(lv.ops.halt - P::ONES);
    }

    fn permutation_pairs(&self) -> Vec<PermutationPair> {
        vec![
            PermutationPair::singletons(MAP.powers_of_2_in, MAP.powers_of_2_in_permuted),
            PermutationPair::singletons(MAP.powers_of_2_out, MAP.powers_of_2_out_permuted),
        ]
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

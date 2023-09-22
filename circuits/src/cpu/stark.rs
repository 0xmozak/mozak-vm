use std::borrow::Borrow;
use std::marker::PhantomData;

use itertools::izip;
use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use plonky2::hash::hash_types::RichField;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};
use starky::stark::Stark;
use starky::vars::{StarkEvaluationTargets, StarkEvaluationVars};

use super::columns::{CpuColumnsExtended, CpuState, Instruction, OpSelectors};
use super::{add, bitwise, branches, div, ecall, jalr, mul, signed_comparison, sub};
use crate::columns_view::NumberOfColumns;
use crate::cpu::shift;
use crate::program::columns::ProgramRom;
use crate::stark::mozak_stark::PublicInputs;

#[derive(Copy, Clone, Default)]
#[allow(clippy::module_name_repetitions)]
pub struct CpuStark<F, const D: usize> {
    pub _f: PhantomData<F>,
}

impl<P: PackedField> OpSelectors<P> {
    // List of opcodes that manipulated the program counter, instead of
    // straight line incrementing it.
    // Note: ecall is only 'jumping' in the sense that a 'halt'
    // does not bump the PC. It sort-of jumps back to itself.
    pub fn is_jumping(&self) -> P {
        self.beq + self.bge + self.blt + self.bne + self.ecall + self.jalr
    }

    /// List of opcodes that only bump the program counter.
    pub fn is_straightline(&self) -> P { P::ONES - self.is_jumping() }

    /// List of opcodes that work with memory.
    pub fn is_mem_op(&self) -> P { self.sb + self.lbu }
}

/// Ensure that if opcode is straight line, then program counter is incremented
/// by 4.
fn pc_ticks_up<P: PackedField>(
    lv: &CpuState<P>,
    nv: &CpuState<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    yield_constr.constraint_transition(
        lv.inst.ops.is_straightline()
            * (nv.inst.pc - (lv.inst.pc + P::Scalar::from_noncanonical_u64(4))),
    );
}

/// Enforce that selectors of opcode as well as registers are one-hot encoded.
/// Ie exactly one of them should be 1, and all others 0 in each row.
/// See <https://en.wikipedia.org/wiki/One-hot>
fn one_hots<P: PackedField>(inst: &Instruction<P>, yield_constr: &mut ConstraintConsumer<P>) {
    one_hot(inst.ops, yield_constr);
    one_hot(inst.rs1_select, yield_constr);
    one_hot(inst.rs2_select, yield_constr);
    one_hot(inst.rd_select, yield_constr);
}

fn one_hot<P: PackedField, Selectors: Clone + IntoIterator<Item = P>>(
    selectors: Selectors,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    // selectors have value 0 or 1.
    selectors
        .clone()
        .into_iter()
        .for_each(|s| is_binary(yield_constr, s));

    // Only one selector enabled.
    let sum_s_op: P = selectors.into_iter().sum();
    yield_constr.constraint(P::ONES - sum_s_op);
}

/// Ensure an expression only takes on values 0 or 1.
/// This doubles the degree of the provided expression `x`,
/// so as long as we are targeting degree <= 3,
/// this should only be called with at most linear expressions.
pub fn is_binary<P: PackedField>(yield_constr: &mut ConstraintConsumer<P>, x: P) {
    yield_constr.constraint(x * (P::ONES - x));
}

/// Ensure an expression only takes on values 0 or 1 for transition rows.
///
/// That's useful for differences between `local_values` and `next_values`, like
/// a clock tick.
fn is_binary_transition<P: PackedField>(yield_constr: &mut ConstraintConsumer<P>, x: P) {
    yield_constr.constraint_transition(x * (P::ONES - x));
}

/// Ensure clock is ticking up, iff CPU is still running.
fn clock_ticks<P: PackedField>(
    lv: &CpuState<P>,
    nv: &CpuState<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    let clock_diff = nv.clk - lv.clk;
    is_binary_transition(yield_constr, clock_diff);
    yield_constr.constraint_transition(clock_diff - lv.is_running);
}

/// Register 0 is always 0
fn r0_always_0<P: PackedField>(lv: &CpuState<P>, yield_constr: &mut ConstraintConsumer<P>) {
    yield_constr.constraint(lv.regs[0]);
}

/// This function ensures that for each unique value present in
/// the instruction column the [`filter`] flag is `1`. This is done by comparing
/// the local row and the next row values.
/// As the result, `filter` marks all duplicated instructions with `0`.
fn check_permuted_inst_cols<P: PackedField>(
    lv: &ProgramRom<P>,
    nv: &ProgramRom<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    yield_constr.constraint(lv.filter * (lv.filter - P::ONES));
    yield_constr.constraint_first_row(lv.filter - P::ONES);

    for (lv_col, nv_col) in izip![lv.inst, nv.inst] {
        yield_constr.constraint((nv.filter - P::ONES) * (lv_col - nv_col));
    }
}

/// Only the destination register can change its value.  
/// All other registers must keep the same value as in the previous row.
fn only_rd_changes<P: PackedField>(
    lv: &CpuState<P>,
    nv: &CpuState<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    // Note: we could skip 0, because r0 is always 0.
    // But we keep it to make it easier to reason about the code.
    (0..32).for_each(|reg| {
        yield_constr.constraint_transition(
            (P::ONES - lv.inst.rd_select[reg]) * (lv.regs[reg] - nv.regs[reg]),
        );
    });
}

/// The destination register should change to `dst_value`.
fn rd_assigned_correctly<P: PackedField>(
    lv: &CpuState<P>,
    nv: &CpuState<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    // Note: we skip 0 here, because it's already forced to 0 permanently by
    // `r0_always_0`
    (1..32).for_each(|reg| {
        yield_constr
            .constraint_transition((lv.inst.rd_select[reg]) * (lv.dst_value - nv.regs[reg]));
    });
}

/// First operand should be assigned with the value of the designated register.
fn populate_op1_value<P: PackedField>(lv: &CpuState<P>, yield_constr: &mut ConstraintConsumer<P>) {
    yield_constr.constraint(
        lv.op1_value
            // Note: we could skip 0, because r0 is always 0.
            // But we keep it to make it easier to reason about the code.
            - (0..32)
            .map(|reg| lv.inst.rs1_select[reg] * lv.regs[reg])
            .sum::<P>(),
    );
}

/// Constraints for values in op2, which is the sum of the value of the second
/// operand register and the immediate value (except for branch instructions).
/// This may overflow.
fn populate_op2_value<P: PackedField>(lv: &CpuState<P>, yield_constr: &mut ConstraintConsumer<P>) {
    let wrap_at = CpuState::<P>::shifted(32);
    let ops = &lv.inst.ops;
    let is_branch_operation = ops.beq + ops.bne + ops.blt + ops.bge;
    let is_shift_operation = ops.sll + ops.srl + ops.sra;

    yield_constr.constraint(is_branch_operation * (lv.op2_value - lv.rs2_value()));
    yield_constr.constraint(is_shift_operation * (lv.op2_value - lv.bitshift.multiplier));
    yield_constr.constraint(
        (P::ONES - is_branch_operation - is_shift_operation)
            * (lv.op2_value_overflowing - lv.inst.imm_value - lv.rs2_value()),
    );
    yield_constr.constraint(
        (P::ONES - is_branch_operation - is_shift_operation)
            * (lv.op2_value_overflowing - lv.op2_value)
            * (lv.op2_value_overflowing - lv.op2_value - wrap_at * ops.is_mem_op()),
    );
}

fn halted<P: PackedField>(
    lv: &CpuState<P>,
    nv: &CpuState<P>,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    let is_halted = P::ONES - lv.is_running;
    is_binary(yield_constr, lv.is_running);
    // TODO: change this when we support segmented proving.
    // Last row must be 'halted', ie no longer is_running.
    yield_constr.constraint_last_row(lv.is_running);

    // Once we stop running, no subsequent row starts running again:
    yield_constr.constraint_transition(is_halted * (nv.is_running - lv.is_running));
    // Halted means that nothing changes anymore:
    for (&lv_entry, &nv_entry) in izip!(lv, nv) {
        yield_constr.constraint_transition(is_halted * (lv_entry - nv_entry));
    }
}

impl<F: RichField + Extendable<D>, const D: usize> Stark<F, D> for CpuStark<F, D> {
    const COLUMNS: usize = CpuColumnsExtended::<F>::NUMBER_OF_COLUMNS;
    // Public inputs: [PC of the first row]
    const PUBLIC_INPUTS: usize = PublicInputs::<F>::NUMBER_OF_COLUMNS;

    #[allow(clippy::similar_names)]
    fn eval_packed_generic<FE, P, const D2: usize>(
        &self,
        vars: StarkEvaluationVars<FE, P, { Self::COLUMNS }, { Self::PUBLIC_INPUTS }>,
        yield_constr: &mut ConstraintConsumer<P>,
    ) where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>, {
        let lv: &CpuColumnsExtended<_> = vars.local_values.borrow();
        let nv: &CpuColumnsExtended<_> = vars.next_values.borrow();
        let public_inputs: &PublicInputs<_> = vars.public_inputs.borrow();

        // Constrain the CPU transition between previous `lv` state and next `nv`
        // state.
        check_permuted_inst_cols(&lv.permuted, &nv.permuted, yield_constr);

        let lv = &lv.cpu;
        let nv = &nv.cpu;

        yield_constr.constraint_first_row(lv.inst.pc - public_inputs.entry_point);
        clock_ticks(lv, nv, yield_constr);
        pc_ticks_up(lv, nv, yield_constr);

        one_hots(&lv.inst, yield_constr);

        // Registers
        r0_always_0(lv, yield_constr);
        only_rd_changes(lv, nv, yield_constr);
        rd_assigned_correctly(lv, nv, yield_constr);
        populate_op1_value(lv, yield_constr);
        populate_op2_value(lv, yield_constr);

        // add constraint
        add::constraints(lv, yield_constr);
        sub::constraints(lv, yield_constr);
        bitwise::constraints(lv, yield_constr);
        branches::comparison_constraints(lv, yield_constr);
        branches::constraints(lv, nv, yield_constr);
        signed_comparison::signed_constraints(lv, yield_constr);
        signed_comparison::slt_constraints(lv, yield_constr);
        shift::constraints(lv, yield_constr);
        div::constraints(lv, yield_constr);
        mul::constraints(lv, yield_constr);
        jalr::constraints(lv, nv, yield_constr);
        ecall::constraints(lv, nv, yield_constr);
        halted(lv, nv, yield_constr);

        // Clock starts at 1. This is to differentiate
        // execution clocks (1 and above) from clk value of `0` which is
        // reserved for any initialisation concerns. e.g. memory initialisation
        // prior to program execution, register initialisation etc.
        yield_constr.constraint_first_row(P::ONES - lv.clk);
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

    use crate::cpu::stark::CpuStark;

    #[test]
    fn test_degree() -> Result<()> {
        const D: usize = 2;
        type C = PoseidonGoldilocksConfig;
        type F = <C as GenericConfig<D>>::F;
        type S = CpuStark<F, D>;

        let stark = S::default();
        test_stark_low_degree(stark)
    }
}

use std::marker::PhantomData;

use itertools::izip;
use mozak_circuits_derive::StarkNameDisplay;
use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};
use starky::evaluation_frame::{StarkEvaluationFrame, StarkFrame};
use starky::stark::Stark;

use super::columns::{
    is_mem_op_extention_target, rs2_value_extension_target, CpuColumnsExtended, CpuState,
    Instruction, OpSelectors,
};
use super::{add, bitwise, branches, div, ecall, jalr, memory, mul, signed_comparison, sub};
use crate::columns_view::{HasNamedColumns, NumberOfColumns};
use crate::cpu::shift;
use crate::program::columns::ProgramRom;
use crate::stark::mozak_stark::PublicInputs;
use crate::stark::utils::{is_binary, is_binary_ext_circuit};

/// A Gadget for CPU Instructions
///
/// Instructions are either handled directly or through cross table lookup
#[derive(Copy, Clone, Default, StarkNameDisplay)]
#[allow(clippy::module_name_repetitions)]
pub struct CpuStark<F, const D: usize> {
    pub _f: PhantomData<F>,
}

impl<F, const D: usize> HasNamedColumns for CpuStark<F, D> {
    type Columns = CpuColumnsExtended<F>;
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
    pub fn is_mem_op(&self) -> P { self.sb + self.lb + self.sh + self.lh + self.sw + self.lw }
}

pub fn add_extension_vec<F: RichField + Extendable<D>, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    targets: Vec<ExtensionTarget<D>>,
) -> ExtensionTarget<D> {
    let mut result = builder.zero_extension();
    for target in targets {
        result = builder.add_extension(result, target);
    }
    result
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

fn pc_ticks_up_circuit<F: RichField + Extendable<D>, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    lv: &CpuState<ExtensionTarget<D>>,
    nv: &CpuState<ExtensionTarget<D>>,
    yield_constr: &mut RecursiveConstraintConsumer<F, D>,
) {
    let four = builder.constant_extension(F::Extension::from_noncanonical_u64(4));
    let lv_inst_pc_add_four = builder.add_extension(lv.inst.pc, four);
    let nv_inst_pc_sub_lv_inst_pc_add_four = builder.sub_extension(nv.inst.pc, lv_inst_pc_add_four);
    let is_jumping = add_extension_vec(builder, vec![
        lv.inst.ops.beq,
        lv.inst.ops.bge,
        lv.inst.ops.blt,
        lv.inst.ops.bne,
        lv.inst.ops.ecall,
        lv.inst.ops.jalr,
    ]);
    let one = builder.one_extension();
    let is_straightline = builder.sub_extension(one, is_jumping);
    let constr = builder.mul_extension(is_straightline, nv_inst_pc_sub_lv_inst_pc_add_four);
    yield_constr.constraint_transition(builder, constr);
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

fn one_hot<P: PackedField, Selectors: Copy + IntoIterator<Item = P>>(
    selectors: Selectors,
    yield_constr: &mut ConstraintConsumer<P>,
) {
    // selectors have value 0 or 1.
    selectors
        .into_iter()
        .for_each(|s| is_binary(yield_constr, s));

    // Only one selector enabled.
    let sum_s_op: P = selectors.into_iter().sum();
    yield_constr.constraint(P::ONES - sum_s_op);
}

fn one_hots_circuit<F: RichField + Extendable<D>, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    inst: &Instruction<ExtensionTarget<D>>,
    yield_constr: &mut RecursiveConstraintConsumer<F, D>,
) {
    one_hot_circuit(builder, &inst.ops.iter().as_slice().to_vec(), yield_constr);
    one_hot_circuit(builder, &inst.rs1_select.to_vec(), yield_constr);
    one_hot_circuit(builder, &inst.rs2_select.to_vec(), yield_constr);
    one_hot_circuit(builder, &inst.rd_select.to_vec(), yield_constr);
}

fn one_hot_circuit<F: RichField + Extendable<D>, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    selectors: &Vec<ExtensionTarget<D>>,
    yield_constr: &mut RecursiveConstraintConsumer<F, D>,
) {
    for selector in selectors {
        is_binary_ext_circuit(builder, *selector, yield_constr);
    }
    let one = builder.one_extension();
    let sum_s_op = selectors.iter().fold(builder.zero_extension(), |acc, s| {
        builder.add_extension(acc, *s)
    });
    let one_sub_sum_s_op = builder.sub_extension(one, sum_s_op);
    yield_constr.constraint(builder, one_sub_sum_s_op);
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

fn clock_ticks_circuit<F: RichField + Extendable<D>, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    lv: &CpuState<ExtensionTarget<D>>,
    nv: &CpuState<ExtensionTarget<D>>,
    yield_constr: &mut RecursiveConstraintConsumer<F, D>,
) {
    let clock_diff = builder.sub_extension(nv.clk, lv.clk);
    let one = builder.one_extension();
    let one_sub_clock_diff = builder.sub_extension(one, clock_diff);
    let clock_diff_mul_one_sub_clock_diff = builder.mul_extension(clock_diff, one_sub_clock_diff);
    yield_constr.constraint_transition(builder, clock_diff_mul_one_sub_clock_diff);
    let clock_diff_sub_lv_is_running = builder.sub_extension(clock_diff, lv.is_running);
    yield_constr.constraint_transition(builder, clock_diff_sub_lv_is_running);
}

/// Register 0 is always 0
fn r0_always_0<P: PackedField>(lv: &CpuState<P>, yield_constr: &mut ConstraintConsumer<P>) {
    yield_constr.constraint(lv.regs[0]);
}

fn r0_always_0_circuit<F: RichField + Extendable<D>, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    lv: &CpuState<ExtensionTarget<D>>,
    yield_constr: &mut RecursiveConstraintConsumer<F, D>,
) {
    yield_constr.constraint(builder, lv.regs[0]);
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

pub fn check_permuted_inst_cols_circuit<F: RichField + Extendable<D>, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    lv: &ProgramRom<ExtensionTarget<D>>,
    nv: &ProgramRom<ExtensionTarget<D>>,
    yield_constr: &mut RecursiveConstraintConsumer<F, D>,
) {
    let one = builder.one_extension();
    let lv_filter_sub_one = builder.sub_extension(lv.filter, one);
    let lv_filter_mul_lv_filter_sub_one = builder.mul_extension(lv.filter, lv_filter_sub_one);
    yield_constr.constraint(builder, lv_filter_mul_lv_filter_sub_one);
    yield_constr.constraint_first_row(builder, lv_filter_sub_one);

    for (lv_col, nv_col) in izip![lv.inst, nv.inst] {
        let nv_filter_sub_one = builder.sub_extension(nv.filter, one);
        let lv_col_sub_nv_col = builder.sub_extension(lv_col, nv_col);
        let nv_filter_sub_one_mul_lv_col_sub_nv_col =
            builder.mul_extension(nv_filter_sub_one, lv_col_sub_nv_col);
        yield_constr.constraint(builder, nv_filter_sub_one_mul_lv_col_sub_nv_col);
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

fn only_rd_changes_circuit<F: RichField + Extendable<D>, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    lv: &CpuState<ExtensionTarget<D>>,
    nv: &CpuState<ExtensionTarget<D>>,
    yield_constr: &mut RecursiveConstraintConsumer<F, D>,
) {
    let one = builder.one_extension();
    for reg in 0..32 {
        let lv_regs_sub_nv_regs = builder.sub_extension(lv.regs[reg], nv.regs[reg]);
        let one_sub_lv_inst_rd_select = builder.sub_extension(one, lv.inst.rd_select[reg]);
        let constr = builder.mul_extension(one_sub_lv_inst_rd_select, lv_regs_sub_nv_regs);
        yield_constr.constraint_transition(builder, constr);
    }
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

fn rd_assigned_correctly_circuit<F: RichField + Extendable<D>, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    lv: &CpuState<ExtensionTarget<D>>,
    nv: &CpuState<ExtensionTarget<D>>,
    yield_constr: &mut RecursiveConstraintConsumer<F, D>,
) {
    for reg in 1..32 {
        let lv_inst_rd_select = lv.inst.rd_select[reg];
        let lv_dst_value_sub_nv_regs = builder.sub_extension(lv.dst_value, nv.regs[reg]);
        let constr = builder.mul_extension(lv_inst_rd_select, lv_dst_value_sub_nv_regs);
        yield_constr.constraint_transition(builder, constr);
    }
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

fn populate_op1_value_circuit<F: RichField + Extendable<D>, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    lv: &CpuState<ExtensionTarget<D>>,
    yield_constr: &mut RecursiveConstraintConsumer<F, D>,
) {
    let mut op1_value = builder.zero_extension();
    for reg in 0..32 {
        let lv_inst_rs1_select = lv.inst.rs1_select[reg];
        let lv_regs = lv.regs[reg];
        let lv_inst_rs1_select_mul_lv_regs = builder.mul_extension(lv_inst_rs1_select, lv_regs);
        op1_value = builder.add_extension(op1_value, lv_inst_rs1_select_mul_lv_regs);
    }
    let lv_op1_value_sub_op1_value = builder.sub_extension(lv.op1_value, op1_value);
    yield_constr.constraint(builder, lv_op1_value_sub_op1_value);
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

fn populate_op2_value_circuit<F: RichField + Extendable<D>, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    lv: &CpuState<ExtensionTarget<D>>,
    yield_constr: &mut RecursiveConstraintConsumer<F, D>,
) {
    let wrap_at = builder.constant_extension(F::Extension::from_canonical_u64(1 << 32));
    let ops = &lv.inst.ops;
    let is_branch_operation = add_extension_vec(builder, vec![ops.beq, ops.bne, ops.blt, ops.bge]);
    let is_shift_operation = add_extension_vec(builder, vec![ops.sll, ops.srl, ops.sra]);

    let rs2_value = rs2_value_extension_target(builder, lv);
    let lv_op2_value_sub_rs2_value = builder.sub_extension(lv.op2_value, rs2_value);
    let is_branch_op_mul_lv_op2_value_sub_rs2_value =
        builder.mul_extension(is_branch_operation, lv_op2_value_sub_rs2_value);
    yield_constr.constraint(builder, is_branch_op_mul_lv_op2_value_sub_rs2_value);

    let op2_sub_bitshift_multiplier = builder.sub_extension(lv.op2_value, lv.bitshift.multiplier);
    let is_shift_op_mul_op2_sub_bitshift_multiplier =
        builder.mul_extension(is_shift_operation, op2_sub_bitshift_multiplier);
    yield_constr.constraint(builder, is_shift_op_mul_op2_sub_bitshift_multiplier);

    let one = builder.one_extension();
    let one_sub_is_branch_operation = builder.sub_extension(one, is_branch_operation);
    let one_sub_is_branch_operation_sub_is_shift_operation =
        builder.sub_extension(one_sub_is_branch_operation, is_shift_operation);
    let op2_value_overflowing_sub_inst_imm_value =
        builder.sub_extension(lv.op2_value_overflowing, lv.inst.imm_value);
    let op2_value_overflowing_sub_inst_imm_value_sub_rs2_value =
        builder.sub_extension(op2_value_overflowing_sub_inst_imm_value, rs2_value);
    let constr = builder.mul_extension(
        one_sub_is_branch_operation_sub_is_shift_operation,
        op2_value_overflowing_sub_inst_imm_value_sub_rs2_value,
    );
    yield_constr.constraint(builder, constr);

    let op2_value_overflowing_sub_op2_value =
        builder.sub_extension(lv.op2_value_overflowing, lv.op2_value);
    let is_mem_op = is_mem_op_extention_target(builder, ops);
    let wrap_at_mul_is_mem_op = builder.mul_extension(wrap_at, is_mem_op);
    let lv_op2_value_overflowing_sub_op2_value_mul_wrap_at_mul_is_mem_op =
        builder.sub_extension(op2_value_overflowing_sub_op2_value, wrap_at_mul_is_mem_op);
    let constr = builder.mul_extension(
        op2_value_overflowing_sub_op2_value,
        lv_op2_value_overflowing_sub_op2_value_mul_wrap_at_mul_is_mem_op,
    );
    let constr = builder.mul_extension(one_sub_is_branch_operation_sub_is_shift_operation, constr);
    yield_constr.constraint(builder, constr);
}

const COLUMNS: usize = CpuColumnsExtended::<()>::NUMBER_OF_COLUMNS;
// Public inputs: [PC of the first row]
const PUBLIC_INPUTS: usize = PublicInputs::<()>::NUMBER_OF_COLUMNS;

impl<F: RichField + Extendable<D>, const D: usize> Stark<F, D> for CpuStark<F, D> {
    type EvaluationFrame<FE, P, const D2: usize> = StarkFrame<P, P::Scalar, COLUMNS, PUBLIC_INPUTS>

    where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>;
    type EvaluationFrameTarget =
        StarkFrame<ExtensionTarget<D>, ExtensionTarget<D>, COLUMNS, PUBLIC_INPUTS>;

    fn eval_packed_generic<FE, P, const D2: usize>(
        &self,
        vars: &Self::EvaluationFrame<FE, P, D2>,
        yield_constr: &mut ConstraintConsumer<P>,
    ) where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>, {
        let lv: &CpuColumnsExtended<_> = vars.get_local_values().into();
        let nv: &CpuColumnsExtended<_> = vars.get_next_values().into();
        let public_inputs: &PublicInputs<_> = vars.get_public_inputs().into();

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

        add::constraints(lv, yield_constr);
        sub::constraints(lv, yield_constr);
        bitwise::constraints(lv, yield_constr);
        branches::comparison_constraints(lv, yield_constr);
        branches::constraints(lv, nv, yield_constr);
        memory::constraints(lv, yield_constr);
        signed_comparison::signed_constraints(lv, yield_constr);
        signed_comparison::slt_constraints(lv, yield_constr);
        shift::constraints(lv, yield_constr);
        div::constraints(lv, yield_constr);
        mul::constraints(lv, yield_constr);
        jalr::constraints(lv, nv, yield_constr);
        ecall::constraints(lv, nv, yield_constr);

        // Clock starts at 2. This is to differentiate
        // execution clocks (2 and above) from
        // clk values `0` and `1` which are reserved for
        // elf initialisation and zero initialisation respectively.
        yield_constr.constraint_first_row(P::ONES + P::ONES - lv.clk);
    }

    fn constraint_degree(&self) -> usize { 3 }

    fn eval_ext_circuit(
        &self,
        builder: &mut CircuitBuilder<F, D>,
        vars: &Self::EvaluationFrameTarget,
        yield_constr: &mut RecursiveConstraintConsumer<F, D>,
    ) {
        let lv: &CpuColumnsExtended<_> = vars.get_local_values().into();
        let nv: &CpuColumnsExtended<_> = vars.get_next_values().into();
        let public_inputs: &PublicInputs<_> = vars.get_public_inputs().into();

        check_permuted_inst_cols_circuit(builder, &lv.permuted, &nv.permuted, yield_constr);

        let lv = &lv.cpu;
        let nv = &nv.cpu;

        let inst_pc_sub_public_inputs_entry_point =
            builder.sub_extension(lv.inst.pc, public_inputs.entry_point);
        yield_constr.constraint_first_row(builder, inst_pc_sub_public_inputs_entry_point);
        clock_ticks_circuit(builder, lv, nv, yield_constr);
        pc_ticks_up_circuit(builder, lv, nv, yield_constr);

        one_hots_circuit(builder, &lv.inst, yield_constr);
        r0_always_0_circuit(builder, lv, yield_constr);
        only_rd_changes_circuit(builder, lv, nv, yield_constr);
        rd_assigned_correctly_circuit(builder, lv, nv, yield_constr);

        populate_op1_value_circuit(builder, lv, yield_constr);
        populate_op2_value_circuit(builder, lv, yield_constr);

        add::constraints_circuit(builder, lv, yield_constr);
        sub::constraints_circuit(builder, lv, yield_constr);
        bitwise::constraints_circuit(builder, lv, yield_constr);
        branches::comparison_constraints_circuit(builder, lv, yield_constr);
        branches::constraints_circuit(builder, lv, nv, yield_constr);
        memory::constraints_circuit(builder, lv, yield_constr);
        signed_comparison::signed_constraints_circuit(builder, lv, yield_constr);
        signed_comparison::slt_constraints_circuit(builder, lv, yield_constr);
        shift::constraints_circuit(builder, lv, yield_constr);
        div::constraints_circuit(builder, lv, yield_constr);
        mul::constraints_circuit(builder, lv, yield_constr);
        jalr::constraints_circuit(builder, lv, nv, yield_constr);
        ecall::constraints_circuit(builder, lv, nv, yield_constr);

        let two = builder.two_extension();
        let two_sub_lv_clk = builder.sub_extension(two, lv.clk);
        yield_constr.constraint_first_row(builder, two_sub_lv_clk);
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use plonky2::plonk::config::{GenericConfig, Poseidon2GoldilocksConfig};
    use starky::stark_testing::{test_stark_circuit_constraints, test_stark_low_degree};

    use crate::cpu::stark::CpuStark;

    #[test]
    fn test_degree() -> Result<()> {
        const D: usize = 2;
        type C = Poseidon2GoldilocksConfig;
        type F = <C as GenericConfig<D>>::F;
        type S = CpuStark<F, D>;

        let stark = S::default();
        test_stark_low_degree(stark)
    }

    #[test]
    fn test_circuit() -> Result<()> {
        const D: usize = 2;
        type C = Poseidon2GoldilocksConfig;
        type F = <C as GenericConfig<D>>::F;
        type S = CpuStark<F, D>;

        let stark = S::default();
        test_stark_circuit_constraints::<F, C, S, D>(stark)?;

        Ok(())
    }
}

use std::marker::PhantomData;
use std::process::Output;

use derive_more::Sub;
use expr::{Expr, ExprBuilder, StarkFrameTyped};
use mozak_circuits_derive::StarkNameDisplay;
use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use plonky2::gates::public_input;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};
use starky::evaluation_frame::{StarkEvaluationFrame, StarkFrame};
use starky::stark::Stark;

use super::columns::{CpuState, Instruction, OpSelectors};
use super::{add, bitwise, branches, div, ecall, jalr, memory, mul, signed_comparison, sub};
use crate::columns_view::{HasNamedColumns, NumberOfColumns};
use crate::cpu::shift;
use crate::expr::{build_ext, build_packed, ConstraintBuilder};
use crate::stark::mozak_stark::PublicInputs;
use crate::stark::utils::{is_binary, is_binary_ext_circuit};
use core::ops::Add;

/// A Gadget for CPU Instructions
///
/// Instructions are either handled directly or through cross table lookup
#[derive(Copy, Clone, Default, StarkNameDisplay)]
#[allow(clippy::module_name_repetitions)]
pub struct CpuStark<F, const D: usize> {
    pub _f: PhantomData<F>,
}

impl<F, const D: usize> HasNamedColumns for CpuStark<F, D> {
    type Columns = CpuState<F>;
}

impl<P: Copy + Add<Output = P>> OpSelectors<P> where 
    i64: Sub<P, Output = P>,
{
    // List of opcodes that manipulated the program counter, instead of
    // straight line incrementing it.
    // Note: ecall is only 'jumping' in the sense that a 'halt'
    // does not bump the PC. It sort-of jumps back to itself.
    pub fn is_jumping(&self) -> P {
        self.beq + self.bge + self.blt + self.bne + self.ecall + self.jalr
    }

    /// List of opcodes that only bump the program counter.
    pub fn is_straightline(&self) -> P { 1 - self.is_jumping() }

    /// List of opcodes that work with memory.
    pub fn is_mem_op(&self) -> P { self.sb + self.lb + self.sh + self.lh + self.sw + self.lw }
}

/// Ensure that if opcode is straight line, then program counter is incremented
/// by 4.
fn pc_ticks_up<'a, P: Copy>(
    lv: &CpuState<Expr<'a, P>>,
    nv: &CpuState<Expr<'a, P>>,
    cb: &mut ConstraintBuilder<Expr<'a, P>>,
) {
    cb.transition(
         lv.inst.ops.is_straightline()
            * (nv.inst.pc - (lv.inst.pc + 4)),
    );
}

/// Enforce that selectors of opcode as well as registers are one-hot encoded.
/// Ie exactly one of them should be 1, and all others 0 in each row.
/// See <https://en.wikipedia.org/wiki/One-hot>
fn one_hots<P: PackedField>(inst: &Instruction<P>, yield_constr: &mut ConstraintConsumer<P>) {
    one_hot(inst.ops, yield_constr);
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
fn clock_ticks<'a, P: Copy>(
    lv: &CpuState<Expr<'a, P>>,
    nv: &CpuState<Expr<'a, P>>,
    cb: &mut ConstraintBuilder<Expr<'a, P>>,
) {
    let clock_diff = nv.clk - lv.clk;
    cb.transition(clock_diff.is_binary());
    cb.transition(clock_diff - lv.is_running);
}

/// Constraints for values in op2, which is the sum of the value of the second
/// operand register and the immediate value (except for branch instructions).
/// This may overflow.
// fn populate_op2_value<P: PackedField>(lv: &CpuState<P>, yield_constr: &mut ConstraintConsumer<P>) {
//     let wrap_at = CpuState::<P>::shifted(32);
//     let ops = &lv.inst.ops;
//     let is_branch_operation = ops.beq + ops.bne + ops.blt + ops.bge;
//     let is_shift_operation = ops.sll + ops.srl + ops.sra;

//     yield_constr.constraint(is_branch_operation * (lv.op2_value - lv.op2_value_raw));
//     yield_constr.constraint(is_shift_operation * (lv.op2_value - lv.bitshift.multiplier));
//     yield_constr.constraint(
//         (P::ONES - is_branch_operation - is_shift_operation)
//             * (lv.op2_value_overflowing - lv.inst.imm_value - lv.op2_value_raw),
//     );
//     yield_constr.constraint(
//         (P::ONES - is_branch_operation - is_shift_operation)
//             * (lv.op2_value_overflowing - lv.op2_value)
//             * (lv.op2_value_overflowing - lv.op2_value - wrap_at * ops.is_mem_op()),
//     );
// }

const COLUMNS: usize = CpuState::<()>::NUMBER_OF_COLUMNS;
// Public inputs: [PC of the first row]
const PUBLIC_INPUTS: usize = PublicInputs::<()>::NUMBER_OF_COLUMNS;

 
fn generate_constraints<'a, T: Copy>(
    vars: &StarkFrameTyped<CpuState<Expr<'a, T>>, PublicInputs<Expr<'a, T>>>,
) -> ConstraintBuilder<Expr<'a, T>> {
    let lv = &vars.local_values;
    let nv = &vars.next_values;
    let public_inputs = vars.public_inputs;
    let mut constraints = ConstraintBuilder::default();

    constraints.first_row(lv.inst.pc - public_inputs.entry_point);
    clock_ticks(lv, nv, &mut constraints);
    pc_ticks_up(lv, nv, &mut constraints);

    //     one_hots(&lv.inst, &mut constraints);

    //     // Registers
    //     populate_op2_value(lv, &mut constraints);

    //     add::constraints(lv, &mut constraints);
    //     sub::constraints(lv, &mut constraints);
    //     bitwise::constraints(lv, &mut constraints);
    //     branches::comparison_constraints(lv, &mut constraints);
    //     branches::constraints(lv, nv, &mut constraints);
    //     memory::constraints(lv, &mut constraints);
    //     signed_comparison::signed_constraints(lv, &mut constraints);
    //     signed_comparison::slt_constraints(lv, &mut constraints);
    //     shift::constraints(lv, &mut constraints);
    //     div::constraints(lv, &mut constraints);
    //     mul::constraints(lv, &mut constraints);
    //     jalr::constraints(lv, nv, &mut constraints);
    //     ecall::constraints(lv, nv, &mut constraints);

    //     // Clock starts at 2. This is to differentiate
    //     // execution clocks (2 and above) from
    //     // clk values `0` and `1` which are reserved for
    //     // elf initialisation and zero initialisation respectively.
    //     constraints.first_row(P::ONES + P::ONES - lv.clk);

    constraints
}

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
        constraint_consumer: &mut ConstraintConsumer<P>,
    ) where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>, {
        let expr_builder = ExprBuilder::default();
        // TODO(Matthias): handle conversion of public inputs less uglily.
        let public_inputs: [P::Scalar; PUBLIC_INPUTS] = vars.get_public_inputs().try_into().unwrap();
        let vars: StarkFrame<P, P, COLUMNS, PUBLIC_INPUTS> = 
            StarkFrame::from_values(
                vars.get_local_values(),
                 vars.get_next_values(),
                &public_inputs.map(P::from),
            )
        ;
        let vars: StarkFrameTyped<CpuState<Expr<'_, P>>, PublicInputs<_>> = expr_builder.to_typed_starkframe(&vars);
        let constraints = generate_constraints(&vars);
        build_packed(constraints, constraint_consumer);
    }

    fn constraint_degree(&self) -> usize { 3 }

    fn eval_ext_circuit(
        &self,
        circuit_builder: &mut CircuitBuilder<F, D>,
        vars: &Self::EvaluationFrameTarget,
        constraint_consumer: &mut RecursiveConstraintConsumer<F, D>,
    ) {
        let expr_builder = ExprBuilder::default();
        let constraints = generate_constraints(&expr_builder.to_typed_starkframe(vars));
        build_ext(constraints, circuit_builder, constraint_consumer);
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

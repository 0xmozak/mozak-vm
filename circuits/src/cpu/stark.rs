use std::marker::PhantomData;

use expr::{Expr, ExprBuilder, StarkFrameTyped};
use mozak_circuits_derive::StarkNameDisplay;
use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};
use starky::evaluation_frame::StarkFrame;
use starky::stark::Stark;

use super::columns::{CpuState, Instruction};
use super::{add, bitwise, branches, div, ecall, jalr, memory, mul, signed_comparison, sub};
use crate::columns_view::{HasNamedColumns, NumberOfColumns};
use crate::cpu::shift;
use crate::expr::{build_ext, build_packed, ConstraintBuilder};
use crate::stark::mozak_stark::PublicInputs;

/// A Gadget for CPU Instructions
///
/// Instructions are either handled directly or through cross table lookup
#[derive(Copy, Clone, Default, StarkNameDisplay)]
#[allow(clippy::module_name_repetitions)]
pub struct CpuStark<F, const D: usize> {
    pub _f: PhantomData<F>,
    pub standalone_proving: bool,
}

impl<F, const D: usize> HasNamedColumns for CpuStark<F, D> {
    type Columns = CpuState<F>;
}

/// Ensure that if opcode is straight line, then program counter is incremented
/// by 4.
fn pc_ticks_up<'a, P: Copy>(
    lv: &CpuState<Expr<'a, P>>,
    nv: &CpuState<Expr<'a, P>>,
    cb: &mut ConstraintBuilder<Expr<'a, P>>,
) {
    cb.transition(lv.inst.ops.is_straightline() * (nv.inst.pc - (lv.inst.pc + 4)));
}

/// Enforce that selectors of opcode are one-hot encoded.
/// Ie exactly one of them should be 1, and all others 0 in each row.
/// See <https://en.wikipedia.org/wiki/One-hot>
fn one_hots<'a, P: Copy>(
    inst: &'a Instruction<Expr<'a, P>>,
    cb: &mut ConstraintBuilder<Expr<'a, P>>,
) {
    one_hot(inst.ops, cb);
}

fn one_hot<'a, P: Copy, Selectors: Copy + IntoIterator<Item = Expr<'a, P>>>(
    selectors: Selectors,
    cb: &mut ConstraintBuilder<Expr<'a, P>>,
) {
    // selectors have value 0 or 1.
    selectors.into_iter().for_each(|s| cb.always(s.is_binary()));

    // Only one selector enabled.
    let sum_s_op: Expr<'a, P> = selectors.into_iter().sum();
    cb.always(1 - sum_s_op);
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
fn populate_op2_value<'a, P: Copy>(
    lv: &CpuState<Expr<'a, P>>,
    cb: &mut ConstraintBuilder<Expr<'a, P>>,
) {
    let ops = &lv.inst.ops;
    let is_branch_operation = ops.beq + ops.bne + ops.blt + ops.bge;
    let is_shift_operation = ops.sll + ops.srl + ops.sra;

    cb.always(is_branch_operation * (lv.op2_value - lv.op2_value_raw));
    cb.always(is_shift_operation * (lv.op2_value - lv.bitshift.multiplier));
    cb.always(
        (1 - is_branch_operation - is_shift_operation)
            * (lv.op2_value_overflowing - lv.inst.imm_value - lv.op2_value_raw),
    );
    cb.always(
        (1 - is_branch_operation - is_shift_operation)
            * (lv.op2_value_overflowing - lv.op2_value)
            * (lv.op2_value_overflowing - lv.op2_value - (1 << 32) * ops.is_mem_op()),
    );
}

const COLUMNS: usize = CpuState::<()>::NUMBER_OF_COLUMNS;
// Public inputs: [PC of the first row]
const PUBLIC_INPUTS: usize = PublicInputs::<()>::NUMBER_OF_COLUMNS;

fn generate_constraints<'a, T: Copy>(
    vars: &'a StarkFrameTyped<CpuState<Expr<'a, T>>, PublicInputs<Expr<'a, T>>>,
) -> ConstraintBuilder<Expr<'a, T>> {
    let lv = &vars.local_values;
    let nv = &vars.next_values;
    let public_inputs = vars.public_inputs;
    let mut constraints = ConstraintBuilder::default();

    constraints.first_row(lv.inst.pc - public_inputs.entry_point);
    clock_ticks(lv, nv, &mut constraints);
    pc_ticks_up(lv, nv, &mut constraints);

    one_hots(&lv.inst, &mut constraints);

    // Registers
    populate_op2_value(lv, &mut constraints);

    add::constraints(lv, &mut constraints);
    sub::constraints(lv, &mut constraints);
    bitwise::constraints(lv, &mut constraints);
    branches::comparison_constraints(lv, &mut constraints);
    branches::constraints(lv, nv, &mut constraints);
    memory::constraints(lv, &mut constraints);
    signed_comparison::signed_constraints(lv, &mut constraints);
    signed_comparison::slt_constraints(lv, &mut constraints);
    shift::constraints(lv, &mut constraints);
    div::constraints(lv, &mut constraints);
    mul::constraints(lv, &mut constraints);
    jalr::constraints(lv, nv, &mut constraints);
    ecall::constraints(lv, nv, &mut constraints);

    // Clock starts at 2. This is to differentiate
    // execution clocks (2 and above) from
    // clk values `0` and `1` which are reserved for
    // elf initialisation and zero initialisation respectively.
    constraints.first_row(2 - lv.clk);

    constraints
}

impl<F: RichField + Extendable<D>, const D: usize> Stark<F, D> for CpuStark<F, D> {
    type EvaluationFrame<FE, P, const D2: usize> = StarkFrame<P, P::Scalar, COLUMNS, PUBLIC_INPUTS>

    where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>;
    type EvaluationFrameTarget =
        StarkFrame<ExtensionTarget<D>, ExtensionTarget<D>, COLUMNS, PUBLIC_INPUTS>;

    fn requires_ctls(&self) -> bool { !self.standalone_proving }

    fn eval_packed_generic<FE, P, const D2: usize>(
        &self,
        vars: &Self::EvaluationFrame<FE, P, D2>,
        constraint_consumer: &mut ConstraintConsumer<P>,
    ) where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>, {
        let expr_builder = ExprBuilder::default();
        let vars = expr_builder.to_typed_starkframe(vars);
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
        let vars = expr_builder.to_typed_starkframe(vars);
        let constraints = generate_constraints(&vars);
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

        let stark = S {
            standalone_proving: true,
            ..S::default()
        };
        test_stark_low_degree(stark)
    }

    #[test]
    fn test_circuit() -> Result<()> {
        const D: usize = 2;
        type C = Poseidon2GoldilocksConfig;
        type F = <C as GenericConfig<D>>::F;
        type S = CpuStark<F, D>;

        let stark = S {
            standalone_proving: true,
            ..S::default()
        };
        test_stark_circuit_constraints::<F, C, S, D>(stark)?;

        Ok(())
    }
}

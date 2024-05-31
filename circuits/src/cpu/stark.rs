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

use super::columns::{CpuState, OpSelectors};
use super::{bitwise, branches, div, ecall, jalr, memory, mul, signed_comparison, sub};
use crate::columns_view::{HasNamedColumns, NumberOfColumns};
use crate::cpu::shift;
use crate::expr::{build_ext, build_packed, ConstraintBuilder, GenerateConstraints};
use crate::unstark::NoColumns;

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

/// Ensure that if opcode is straight line, then program counter is incremented
/// by 4.
fn pc_ticks_up<'a, P: Copy>(lv: &CpuState<Expr<'a, P>>, cb: &mut ConstraintBuilder<Expr<'a, P>>) {
    cb.transition(lv.inst.ops.is_straightline() * (lv.new_pc - (lv.inst.pc + 4)));
}

/// Enforce that selectors of opcode are one-hot encoded.
/// Ie exactly one of them should be 1, and all others 0 in each row.
/// See <https://en.wikipedia.org/wiki/One-hot>
fn binary_selectors<'a, P: Copy>(
    ops: &OpSelectors<Expr<'a, P>>,
    cb: &mut ConstraintBuilder<Expr<'a, P>>,
) {
    // selectors have value 0 or 1.
    ops.into_iter().for_each(|s| cb.always(s.is_binary()));

    // Only at most one selector enabled.
    cb.always(ops.is_running().is_binary());
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
const PUBLIC_INPUTS: usize = 0;

impl<'a, F, T: Copy, U, const D: usize>
    GenerateConstraints<'a, T, CpuState<Expr<'a, T>>, NoColumns<U>> for CpuStark<F, { D }>
{
    fn generate_constraints(
        vars: &StarkFrameTyped<CpuState<Expr<'a, T>>, NoColumns<U>>,
    ) -> ConstraintBuilder<Expr<'a, T>> {
        let lv = &vars.local_values;
        let mut constraints = ConstraintBuilder::default();

        pc_ticks_up(lv, &mut constraints);

        binary_selectors(&lv.inst.ops, &mut constraints);

        // Registers
        populate_op2_value(lv, &mut constraints);

        // ADD is now handled by its own table.
        constraints.always(lv.inst.ops.add);
        sub::constraints(lv, &mut constraints);
        bitwise::constraints(lv, &mut constraints);
        branches::comparison_constraints(lv, &mut constraints);
        branches::constraints(lv, &mut constraints);
        memory::constraints(lv, &mut constraints);
        signed_comparison::signed_constraints(lv, &mut constraints);
        signed_comparison::slt_constraints(lv, &mut constraints);
        shift::constraints(lv, &mut constraints);
        div::constraints(lv, &mut constraints);
        mul::constraints(lv, &mut constraints);
        jalr::constraints(lv, &mut constraints);
        ecall::constraints(lv, &mut constraints);

        constraints
    }
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
        let constraints = Self::generate_constraints(&expr_builder.to_typed_starkframe(vars));
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
        let constraints = Self::generate_constraints(&expr_builder.to_typed_starkframe(vars));
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

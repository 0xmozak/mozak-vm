use expr::{BinOp, Evaluator, Expr, ExprBuilder};
use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::RecursiveConstraintConsumer;

use crate::memory::columns::{is_executed_ext_circuit, Memory};
use crate::stark::utils::is_binary_ext_circuit;

struct CircuitBuilderEvaluator<'a, F, const D: usize>
where
    F: RichField,
    F: Extendable<D>, {
    builder: &'a mut CircuitBuilder<F, D>,
}

impl<'a, F, const D: usize> Evaluator<ExtensionTarget<D>> for CircuitBuilderEvaluator<'a, F, D>
where
    F: RichField,
    F: Extendable<D>,
{
    fn bin_op(
        &mut self,
        op: &BinOp,
        left: ExtensionTarget<D>,
        right: ExtensionTarget<D>,
    ) -> ExtensionTarget<D> {
        match op {
            BinOp::Add => self.builder.add_extension(left, right),
            BinOp::Sub => self.builder.sub_extension(left, right),
            BinOp::Mul => self.builder.mul_extension(left, right),
            BinOp::Div => self.builder.div_extension(left, right),
        }
    }
}

pub struct ConstraintBuilderExt<'a, F, const D: usize>
where
    F: RichField,
    F: Extendable<D>, {
    yield_constr: &'a mut RecursiveConstraintConsumer<F, D>,
    circuit_builder: &'a mut CircuitBuilder<F, D>,
}

impl<'a, F, const D: usize> ConstraintBuilderExt<'a, F, D>
where
    F: RichField,
    F: Extendable<D>,
{
    pub fn new(
        yield_constr: &'a mut RecursiveConstraintConsumer<F, D>,
        circuit_builder: &'a mut CircuitBuilder<F, D>,
    ) -> Self {
        Self {
            yield_constr,
            circuit_builder,
        }
    }

    pub fn constraint_first_row(&mut self, constraints: Expr<'_, ExtensionTarget<D>>) {
        let mut evaluator = CircuitBuilderEvaluator {
            builder: self.circuit_builder,
        };
        let built_constraints = evaluator.eval(constraints);
        self.yield_constr
            .constraint_first_row(self.circuit_builder, built_constraints);
    }

    pub fn constraint(&mut self, constraints: Expr<'_, ExtensionTarget<D>>) {
        let mut evaluator = CircuitBuilderEvaluator {
            builder: self.circuit_builder,
        };
        let built_constraints = evaluator.eval(constraints);
        self.yield_constr
            .constraint(self.circuit_builder, built_constraints);
    }

    pub fn constraint_transition(&mut self, constraints: Expr<'_, ExtensionTarget<D>>) {
        let mut evaluator = CircuitBuilderEvaluator {
            builder: self.circuit_builder,
        };
        let built_constraints = evaluator.eval(constraints);
        self.yield_constr
            .constraint_transition(self.circuit_builder, built_constraints);
    }

    pub fn is_binary(&mut self, x: ExtensionTarget<D>) {
        is_binary_ext_circuit(self.circuit_builder, x, self.yield_constr)
    }

    pub fn is_executed(&mut self, values: &Memory<ExtensionTarget<D>>) -> ExtensionTarget<D> {
        is_executed_ext_circuit(self.circuit_builder, values)
    }

    pub fn one(&mut self) -> ExtensionTarget<D> { self.circuit_builder.one_extension() }
}

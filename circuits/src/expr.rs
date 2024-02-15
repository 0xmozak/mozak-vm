use expr::Evaluator;
pub use expr::{BinOp, ExprBuilder, E};
use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::RecursiveConstraintConsumer;

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

pub struct ConstraintBuilder<'a, F, const D: usize>
where
    F: RichField,
    F: Extendable<D>, {
    yield_constr: &'a mut RecursiveConstraintConsumer<F, D>,
    builder: &'a mut CircuitBuilder<F, D>,
}

impl<'a, F, const D: usize> ConstraintBuilder<'a, F, D>
where
    F: RichField,
    F: Extendable<D>,
{
    pub fn new(
        yield_constr: &'a mut RecursiveConstraintConsumer<F, D>,
        builder: &'a mut CircuitBuilder<F, D>,
    ) -> Self {
        Self {
            yield_constr,
            builder,
        }
    }

    pub fn constraint_first_row(&mut self, constraints: E<'_, ExtensionTarget<D>>) {
        let mut evaluator = CircuitBuilderEvaluator {
            builder: self.builder,
        };
        let built_constraints = evaluator.eval(constraints);
        self.yield_constr
            .constraint_first_row(self.builder, built_constraints);
    }

    pub fn constraint(&mut self, constraints: E<'_, ExtensionTarget<D>>) {
        let mut evaluator = CircuitBuilderEvaluator {
            builder: self.builder,
        };
        let built_constraints = evaluator.eval(constraints);
        self.yield_constr
            .constraint(self.builder, built_constraints);
    }

    pub fn constraint_transition(&mut self, constraints: E<'_, ExtensionTarget<D>>) {
        let mut evaluator = CircuitBuilderEvaluator {
            builder: self.builder,
        };
        let built_constraints = evaluator.eval(constraints);
        self.yield_constr
            .constraint_transition(self.builder, built_constraints);
    }
}

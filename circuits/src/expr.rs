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
    fn delta(
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

pub fn constraint_first_row<F, const D: usize>(
    yield_constr: &mut RecursiveConstraintConsumer<F, D>,
    builder: &mut CircuitBuilder<F, D>,
    constraints: E<'_, ExtensionTarget<D>>,
) where
    F: RichField,
    F: Extendable<D>, {
    let mut evaluator = CircuitBuilderEvaluator { builder };
    let built_constraints = constraints.eval(&mut evaluator);
    yield_constr.constraint_first_row(builder, built_constraints);
}

pub fn constraint<F, const D: usize>(
    yield_constr: &mut RecursiveConstraintConsumer<F, D>,
    builder: &mut CircuitBuilder<F, D>,
    constraints: E<'_, ExtensionTarget<D>>,
) where
    F: RichField,
    F: Extendable<D>, {
    let mut evaluator = CircuitBuilderEvaluator { builder };
    let built_constraints = constraints.eval(&mut evaluator);
    yield_constr.constraint(builder, built_constraints);
}

pub fn constraint_transition<F, const D: usize>(
    yield_constr: &mut RecursiveConstraintConsumer<F, D>,
    builder: &mut CircuitBuilder<F, D>,
    constraints: E<'_, ExtensionTarget<D>>,
) where
    F: RichField,
    F: Extendable<D>, {
    let mut evaluator = CircuitBuilderEvaluator { builder };
    let built_constraints = constraints.eval(&mut evaluator);
    yield_constr.constraint_transition(builder, built_constraints);
}

#[cfg(test)]
mod tests {
    use expr::PureEvaluator;

    use super::*;

    #[test]
    fn multiplication() {
        let expr = ExprBuilder::new();

        let a = expr.lit(1);
        let b = expr.lit(2);

        // a and b are cloned behind the scenes
        let c = a * b;
        assert_eq!(c.eval(&mut PureEvaluator::new()), 2);
    }

    #[test]
    fn subtraction() {
        let expr = ExprBuilder::new();

        let a = expr.lit(1);
        let b = expr.lit(2);

        // a and b are cloned behind the scenes
        let c = a - b;
        assert_eq!(c.eval(&mut PureEvaluator::new()), -1);
    }
}

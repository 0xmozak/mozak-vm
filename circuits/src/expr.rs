use std::ops::{Mul, Sub};
use std::rc::Rc;

use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::RecursiveConstraintConsumer;

#[derive(Debug, Clone)]
pub enum Expr<V> {
    ConstExpr {
        val: V,
    },
    SubExpr {
        left: Rc<Expr<V>>,
        right: Rc<Expr<V>>,
    },
    MulExpr {
        left: Rc<Expr<V>>,
        right: Rc<Expr<V>>,
    },
}

impl<V> From<V> for Expr<V> {
    fn from(val: V) -> Self { Self::ConstExpr { val } }
}

impl<const D: usize> Expr<ExtensionTarget<D>> {
    pub fn eval<F>(&self, builder: &mut CircuitBuilder<F, D>) -> ExtensionTarget<D>
    where
        F: RichField,
        F: Extendable<D>, {
        match self {
            Expr::ConstExpr { val } => *val,
            Expr::SubExpr { left, right } => {
                let l = left.eval(builder);
                let r = right.eval(builder);
                builder.sub_extension(l, r)
            }
            Expr::MulExpr { left, right } => {
                let l = left.eval(builder);
                let r = right.eval(builder);
                builder.mul_extension(l, r)
            }
        }
    }
}

impl<V> Sub for Expr<V> {
    type Output = Expr<V>;

    fn sub(self, rhs: Self) -> Self::Output {
        Expr::SubExpr {
            left: Rc::new(self),
            right: Rc::new(rhs),
        }
    }
}

impl<V> Mul for Expr<V> {
    type Output = Expr<V>;

    fn mul(self, rhs: Self) -> Self::Output {
        Expr::MulExpr {
            left: Rc::new(self),
            right: Rc::new(rhs),
        }
    }
}

pub fn constraint_first_row<F, const D: usize>(
    yield_constr: &mut RecursiveConstraintConsumer<F, D>,
    builder: &mut CircuitBuilder<F, D>,
    constraints: Expr<ExtensionTarget<D>>,
) where
    F: RichField,
    F: Extendable<D>, {
    let built_constraints = constraints.eval(builder);
    yield_constr.constraint_first_row(builder, built_constraints);
}

pub fn constraint<F, const D: usize>(
    yield_constr: &mut RecursiveConstraintConsumer<F, D>,
    builder: &mut CircuitBuilder<F, D>,
    constraints: Expr<ExtensionTarget<D>>,
) where
    F: RichField,
    F: Extendable<D>, {
    let built_constraints = constraints.eval(builder);
    yield_constr.constraint(builder, built_constraints);
}

pub fn constraint_transition<F, const D: usize>(
    yield_constr: &mut RecursiveConstraintConsumer<F, D>,
    builder: &mut CircuitBuilder<F, D>,
    constraints: Expr<ExtensionTarget<D>>,
) where
    F: RichField,
    F: Extendable<D>, {
    let built_constraints = constraints.eval(builder);
    yield_constr.constraint_transition(builder, built_constraints);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multiplication() { let _: Expr<_> = Expr::from(1) * Expr::from(2); }
}

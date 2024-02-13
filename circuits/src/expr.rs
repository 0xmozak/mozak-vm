use std::ops::{Mul, Sub};
use std::rc::Rc;

use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::RecursiveConstraintConsumer;

#[derive(Debug, Clone)]
pub enum BinOp {
    Sub,
    Mul
}

#[derive(Debug, Clone)]
pub enum Expr<V> {
    ConstExpr {
        val: V,
    },
    BinOp {
        op: BinOp,
        left: Rc<Expr<V>>,
        right: Rc<Expr<V>>,
    },
}

impl<V> Expr<V> {
    fn sub(left: Self, right: Self) -> Self {
        Self::BinOp {
            op: BinOp::Sub,
            left: Rc::new(left),
            right: Rc::new(right),
        }
    }

    fn mul(left: Self, right: Self) -> Self {
        Self::BinOp {
            op: BinOp::Mul,
            left: Rc::new(left),
            right: Rc::new(right),
        }
    }
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
            Expr::BinOp { op, left, right } => {
                let l = left.eval(builder);
                let r = right.eval(builder);
                match op {
                    BinOp::Sub => builder.sub_extension(l, r),
                    BinOp::Mul => builder.mul_extension(l, r),
                }
            }
        }
    }
}

impl<V> Sub for Expr<V> {
    type Output = Expr<V>;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::Output::sub(self, rhs)
    }
}

impl<'a, V> Sub<&'a Expr<V>> for Expr<V>
where
V: Clone, {
    type Output = Expr<V>;

    fn sub(self, rhs: &'a Self) -> Self::Output {
        Self::Output::sub(self, rhs.clone())
    }
}


impl<'a, V> Sub<Expr<V>> for &'a Expr<V>
where
    V: Clone, {
    type Output = Expr<V>;

    fn sub(self, rhs: Expr<V>) -> Self::Output {
        Self::Output::sub(self.clone(), rhs)
    }
}


impl<'a, V> Sub for &'a Expr<V>
where
    V: Clone, {
    type Output = Expr<V>;

    fn sub(self, rhs: &'a Expr<V>) -> Self::Output {
        Self::Output::sub(self.clone(), rhs.clone())
    }
}


impl<V> Mul for Expr<V> {
    type Output = Expr<V>;

    fn mul(self, rhs: Self) -> Self::Output {
        Self::Output::mul(self, rhs)
    }
}

impl<'a, V> Mul<&'a Expr<V>> for Expr<V>
where
V: Clone, {
    type Output = Expr<V>;

    fn mul(self, rhs: &'a Expr<V>) -> Self::Output {
        Self::Output::mul(self, rhs.clone())
    }
}

impl<'a, V> Mul<Expr<V>> for &'a Expr<V>
where
V: Clone, {
    type Output = Expr<V>;

    fn mul(self, rhs: Expr<V>) -> Self::Output {
        Self::Output::mul(self.clone(), rhs)
    }
}

impl<'a, V> Mul for &'a Expr<V>
where
V: Clone, {
    type Output = Expr<V>;

    fn mul(self, rhs: &'a Expr<V>) -> Self::Output {
        Self::Output::mul(self.clone(), rhs.clone())
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

    // simple evaluator
    fn eval<V>(e: &Expr<V>) -> V
where
V: Copy,
V: Sub<Output = V>,
V: Mul<Output = V>,
 {
        match e {
            Expr::ConstExpr { val } => *val,
            Expr::BinOp { op, left, right } => {
                let a = eval(left);
                let b = eval(right);
                match op {
                    BinOp::Sub => a - b,
                    BinOp::Mul => a * b,
                }
            }
        }
    }

    #[test]
    fn multiplication() {
        let a = Expr::from(1);
        let b = Expr::from(2);

        // a and b are moved
        let _c = a.clone() * b.clone();
        assert_eq!(eval(&_c), 2);

        // a is cloned behind the scenes
        let _c = &a * b.clone();
        assert_eq!(eval(&_c), 2);

        // b is cloned behind the scenes
        let _c = a.clone() * &b;
        assert_eq!(eval(&_c), 2);

        // a and b are cloned behind the scenes
        let _c = &a * &b;
        assert_eq!(eval(&_c), 2);
    }

    #[test]
    fn subtraction() {
        let a = Expr::from(1);
        let b = Expr::from(2);

        // a and b are moved
        let _c = a.clone() - b.clone();
        assert_eq!(eval(&_c), -1);

        // a is cloned behind the scenes
        let _c = &a - b.clone();
        assert_eq!(eval(&_c), -1);

        // b is cloned behind the scenes
        let _c = a.clone() - &b;
        assert_eq!(eval(&_c), -1);

        // a and b are cloned behind the scenes
        let _c = &a - &b;
        assert_eq!(eval(&_c), -1);
    }
}

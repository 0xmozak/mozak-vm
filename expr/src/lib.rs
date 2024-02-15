use std::ops::{Add, Div, Mul, Sub};

use bumpalo::Bump;

// Publicly available struct
#[derive(Clone, Copy)]
pub struct E<'a, V> {
    expr: &'a Expr<'a, V>,
    builder: &'a ExprBuilder,
}

impl<'a, V> Add for E<'a, V> {
    type Output = E<'a, V>;

    fn add(self, rhs: Self) -> Self::Output { self.builder.add(self, rhs) }
}

impl<'a, V> Sub for E<'a, V> {
    type Output = E<'a, V>;

    fn sub(self, rhs: Self) -> Self::Output { self.builder.sub(self, rhs) }
}

impl<'a, V> Mul for E<'a, V> {
    type Output = E<'a, V>;

    fn mul(self, rhs: Self) -> Self::Output { self.builder.mul(self, rhs) }
}

impl<'a, V> Div for E<'a, V> {
    type Output = E<'a, V>;

    fn div(self, rhs: Self) -> Self::Output { self.builder.div(self, rhs) }
}

pub struct ExprBuilder {
    arena: Bump,
}

impl ExprBuilder {
    pub fn new() -> Self { Self { arena: Bump::new() } }

    fn expr<'a, V>(&'a self, expr: &'a mut Expr<'a, V>) -> E<'a, V> {
        // TODO: Consider interning it here
        E {
            expr,
            builder: self,
        }
    }

    // Convenience alias for from
    pub fn lit<'a, V>(&'a self, v: V) -> E<'a, V> { self.expr(self.arena.alloc(Expr::lit(v))) }

    pub fn add<'a, V>(&'a self, left: E<'a, V>, right: E<'a, V>) -> E<'a, V> {
        let left = left.expr;
        let right = right.expr;

        self.expr(self.arena.alloc(Expr::add(left, right)))
    }

    pub fn sub<'a, V>(&'a self, left: E<'a, V>, right: E<'a, V>) -> E<'a, V> {
        let left = left.expr;
        let right = right.expr;

        self.expr(self.arena.alloc(Expr::sub(left, right)))
    }

    pub fn mul<'a, V>(&'a self, left: E<'a, V>, right: E<'a, V>) -> E<'a, V> {
        let left = left.expr;
        let right = right.expr;

        self.expr(self.arena.alloc(Expr::mul(left, right)))
    }

    pub fn div<'a, V>(&'a self, left: E<'a, V>, right: E<'a, V>) -> E<'a, V> {
        let left = left.expr;
        let right = right.expr;

        self.expr(self.arena.alloc(Expr::div(left, right)))
    }
}

#[derive(Debug)]
enum Expr<'a, V> {
    BinOp {
        op: BinOp,
        left: &'a Expr<'a, V>,
        right: &'a Expr<'a, V>,
    },
    Literal {
        value: V,
    },
}

#[derive(Debug)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
}

impl<V> From<V> for Expr<'_, V> {
    fn from(value: V) -> Self { Expr::Literal { value } }
}

// Big step evaluator
fn big_step<'a, E, V>(evaluator: &mut E, expr: &'a Expr<'a, V>) -> V
where
    V: Copy,
    E: ?Sized,
    E: Evaluator<V>, {
    match expr {
        Expr::BinOp { op, left, right } => {
            let l = big_step(evaluator, left);
            let r = big_step(evaluator, right);

            evaluator.bin_op(op, l, r)
        }
        Expr::Literal { value } => *value,
    }
}

pub trait Evaluator<V>
where
    V: Copy, {
    fn bin_op(&mut self, op: &BinOp, left: V, right: V) -> V;
    fn eval<'a>(&mut self, expr: E<'a, V>) -> V {
        // Default eval
        big_step(self, expr.expr)
    }
}

pub struct PureEvaluator {}

impl PureEvaluator {
    pub fn new() -> Self { Self {} }
}

impl<V> Evaluator<V> for PureEvaluator
where
    V: Copy,
    V: Add<Output = V>,
    V: Sub<Output = V>,
    V: Mul<Output = V>,
    V: Div<Output = V>,
{
    fn bin_op(&mut self, op: &BinOp, left: V, right: V) -> V {
        match op {
            BinOp::Add => left + right,
            BinOp::Sub => left - right,
            BinOp::Mul => left * right,
            BinOp::Div => left / right,
        }
    }
}

impl<'a, V> Expr<'a, V> {
    pub fn lit(value: V) -> Self { Expr::Literal { value } }

    pub fn add(left: &'a Expr<'a, V>, right: &'a Expr<'a, V>) -> Self {
        Expr::BinOp {
            op: BinOp::Add,
            left,
            right,
        }
    }

    pub fn sub(left: &'a Expr<'a, V>, right: &'a Expr<'a, V>) -> Self {
        Expr::BinOp {
            op: BinOp::Sub,
            left,
            right,
        }
    }

    pub fn mul(left: &'a Expr<'a, V>, right: &'a Expr<'a, V>) -> Self {
        Expr::BinOp {
            op: BinOp::Mul,
            left,
            right,
        }
    }

    pub fn div(left: &'a Expr<'a, V>, right: &'a Expr<'a, V>) -> Self {
        Expr::BinOp {
            op: BinOp::Div,
            left,
            right,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let expr = ExprBuilder::new();

        let a = expr.lit(7);
        let b = expr.lit(5);

        let mut p = PureEvaluator::new();

        assert_eq!(p.eval(a + b), 12);
        assert_eq!(p.eval(a - b), 2);
        assert_eq!(p.eval(a * b), 35);
        assert_eq!(p.eval(a / b), 1);
    }
}

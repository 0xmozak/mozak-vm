//! Simple library for handling ASTs in Rust

use std::ops::{Add, Div, Mul, Sub};

use bumpalo::Bump;

/// Publicly available struct.  Contains a reference to [`ExprTree`] that is
/// managed by [`ExprBuilder`].
#[derive(Clone, Copy)]
pub struct Expr<'a, V> {
    expr_tree: &'a ExprTree<'a, V>,
    builder: &'a ExprBuilder,
}

impl<'a, V> Add for Expr<'a, V> {
    type Output = Expr<'a, V>;

    fn add(self, rhs: Self) -> Self::Output { self.builder.add(self, rhs) }
}

impl<'a, V> Sub for Expr<'a, V> {
    type Output = Expr<'a, V>;

    fn sub(self, rhs: Self) -> Self::Output { self.builder.sub(self, rhs) }
}

impl<'a, V> Mul for Expr<'a, V> {
    type Output = Expr<'a, V>;

    fn mul(self, rhs: Self) -> Self::Output { self.builder.mul(self, rhs) }
}

impl<'a, V> Div for Expr<'a, V> {
    type Output = Expr<'a, V>;

    fn div(self, rhs: Self) -> Self::Output { self.builder.div(self, rhs) }
}

/// Expression Builder.  Contains a [`Bump`] memory arena that will allocate
/// store all the [`ExprTree`]s.
pub struct ExprBuilder {
    arena: Bump,
}

impl Default for ExprBuilder {
    fn default() -> Self { Self { arena: Bump::new() } }
}

impl ExprBuilder {
    /// Internalise an [`ExprTree`] by moving it to memory allocated by the
    /// [`Bump`] arena owned by [`ExprBuilder`].
    fn intern<'a, V>(&'a self, expr_tree: ExprTree<'a, V>) -> Expr<'a, V> {
        let expr_tree = self.arena.alloc(expr_tree);
        Expr {
            expr_tree,
            builder: self,
        }
    }

    /// Convenience method for creating `BinOp` nodes
    fn bin_op<'a, V>(&'a self, op: BinOp, left: Expr<'a, V>, right: Expr<'a, V>) -> Expr<'a, V> {
        let left = left.expr_tree;
        let right = right.expr_tree;
        let expr_tree = ExprTree::BinOp { op, left, right };

        self.intern(expr_tree)
    }

    /// Create a `Literal` expression
    pub fn lit<'a, V>(&'a self, value: V) -> Expr<'a, V> {
        self.intern(ExprTree::Literal { value })
    }

    /// Create an `Add` expression
    pub fn add<'a, V>(&'a self, left: Expr<'a, V>, right: Expr<'a, V>) -> Expr<'a, V> {
        self.bin_op(BinOp::Add, left, right)
    }

    /// Create a `Sub` expression
    pub fn sub<'a, V>(&'a self, left: Expr<'a, V>, right: Expr<'a, V>) -> Expr<'a, V> {
        self.bin_op(BinOp::Sub, left, right)
    }

    /// Create a `Mul` expression
    pub fn mul<'a, V>(&'a self, left: Expr<'a, V>, right: Expr<'a, V>) -> Expr<'a, V> {
        self.bin_op(BinOp::Mul, left, right)
    }

    /// Create a `div` expression
    pub fn div<'a, V>(&'a self, left: Expr<'a, V>, right: Expr<'a, V>) -> Expr<'a, V> {
        self.bin_op(BinOp::Div, left, right)
    }
}

/// Enum for binary operations
#[derive(Debug)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
}

/// Internal type to represent the expression trees
#[derive(Debug)]
enum ExprTree<'a, V> {
    BinOp {
        op: BinOp,
        left: &'a ExprTree<'a, V>,
        right: &'a ExprTree<'a, V>,
    },
    Literal {
        value: V,
    },
}

/// Big step evaluator
fn big_step<'a, E, V>(evaluator: &mut E, expr: &'a ExprTree<'a, V>) -> V
where
    V: Copy,
    E: ?Sized,
    E: Evaluator<V>, {
    match expr {
        ExprTree::BinOp { op, left, right } => {
            let l = big_step(evaluator, left);
            let r = big_step(evaluator, right);

            evaluator.bin_op(op, l, r)
        }
        ExprTree::Literal { value } => *value,
    }
}

/// Evaluator that can evaluate [`Expr`] to `V`.
pub trait Evaluator<V>
where
    V: Copy, {
    fn bin_op(&mut self, op: &BinOp, left: V, right: V) -> V;
    fn eval<'a>(&mut self, expr: Expr<'a, V>) -> V {
        // Default eval
        big_step(self, expr.expr_tree)
    }
}

/// Default evaluator for pure values.
pub struct PureEvaluator {}

impl Default for PureEvaluator {
    fn default() -> Self { Self {} }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let expr = ExprBuilder::default();

        let a = expr.lit(7);
        let b = expr.lit(5);

        let mut p = PureEvaluator::default();

        assert_eq!(p.eval(a + b), 12);
        assert_eq!(p.eval(a - b), 2);
        assert_eq!(p.eval(a * b), 35);
        assert_eq!(p.eval(a / b), 1);
    }
}

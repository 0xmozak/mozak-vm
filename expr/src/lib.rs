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

    // Create a `One` expression

    pub fn one<'a, V>(&'a self) -> Expr<'a, V> { self.intern(ExprTree::One) }

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

    pub fn is_binary<'a, V>(&'a self, x: Expr<'a, V>) -> Expr<'a, V>
    where
        V: Copy, {
        x * (self.one() - x)
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
    One,
}

impl<V> ExprTree<'_, V>
where
    V: Copy,
{
    fn eval_with<E>(&self, evaluator: &mut E) -> V
    where
        E: Evaluator<V>,
        E: ?Sized, {
        match self {
            ExprTree::BinOp { op, left, right } => {
                let left = left.eval_with(evaluator);
                let right = right.eval_with(evaluator);

                evaluator.bin_op(op, left, right)
            }
            ExprTree::Literal { value } => *value,
            ExprTree::One => evaluator.one(),
        }
    }
}

/// Evaluator that can evaluate [`Expr`] to `V`.
pub trait Evaluator<V>
where
    V: Copy, {
    fn bin_op(&mut self, op: &BinOp, left: V, right: V) -> V;
    fn one(&mut self) -> V;
    fn eval<'a>(&mut self, expr: Expr<'a, V>) -> V { expr.expr_tree.eval_with(self) }
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
    V: From<u8>,
{
    fn bin_op(&mut self, op: &BinOp, left: V, right: V) -> V {
        match op {
            BinOp::Add => left + right,
            BinOp::Sub => left - right,
            BinOp::Mul => left * right,
            BinOp::Div => left / right,
        }
    }

    fn one(&mut self) -> V { 1u8.into() }
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

//! Simple library for handling ASTs for polynomials for ZKP in Rust

use std::ops::{Add, Mul, Sub};

use bumpalo::Bump;

/// Publicly available struct.  Contains a reference to [`ExprTree`] that is
/// managed by [`ExprBuilder`].
#[derive(Clone, Copy, Debug)]
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

/// Expression Builder.  Contains a [`Bump`] memory arena that will allocate
/// store all the [`ExprTree`]s.
#[derive(Debug)]
pub struct ExprBuilder {
    bump: Bump,
}

impl Default for ExprBuilder {
    fn default() -> Self { Self { bump: Bump::new() } }
}

impl ExprBuilder {
    /// Internalise an [`ExprTree`] by moving it to memory allocated by the
    /// [`Bump`] arena owned by [`ExprBuilder`].
    fn intern<'a, V>(&'a self, expr_tree: ExprTree<'a, V>) -> Expr<'a, V> {
        let expr_tree = self.bump.alloc(expr_tree);
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
    pub fn lit<V>(&self, value: V) -> Expr<'_, V> { self.intern(ExprTree::Literal { value }) }

    /// Create a `Constant` expression
    pub fn constant<V>(&self, value: i64) -> Expr<'_, V> {
        self.intern(ExprTree::Constant { value })
    }

    /// Create a `One` expression
    pub fn one<V>(&self) -> Expr<'_, V> { self.constant(1i64) }

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

    pub fn is_binary<'a, V>(&'a self, x: Expr<'a, V>) -> Expr<'a, V>
    where
        V: Copy, {
        x * (self.one() - x)
    }

    pub fn inject_slice<'a, V>(&'a self, items: &'a [V]) -> impl IntoIterator<Item = Expr<'a, V>>
    where
        V: Copy, {
        items.iter().map(|x| self.lit(*x))
    }
}

/// Enum for binary operations
#[derive(Debug)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
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
    Constant {
        value: i64,
    },
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
            ExprTree::Constant { value } => evaluator.constant(*value),
        }
    }
}

/// Evaluator that can evaluate [`Expr`] to `V`.
pub trait Evaluator<V>
where
    V: Copy, {
    fn bin_op(&mut self, op: &BinOp, left: V, right: V) -> V;
    fn constant(&mut self, value: i64) -> V;
    fn eval(&mut self, expr: Expr<'_, V>) -> V { expr.expr_tree.eval_with(self) }
}

/// Default evaluator for pure values.
#[derive(Default)]
pub struct PureEvaluator {}

impl<V> Evaluator<V> for PureEvaluator
where
    V: Copy,
    V: Add<Output = V>,
    V: Sub<Output = V>,
    V: Mul<Output = V>,
    V: From<i64>,
{
    fn bin_op(&mut self, op: &BinOp, left: V, right: V) -> V {
        match op {
            BinOp::Add => left + right,
            BinOp::Sub => left - right,
            BinOp::Mul => left * right,
        }
    }

    fn constant(&mut self, value: i64) -> V { value.into() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let expr = ExprBuilder::default();

        let a = expr.lit(7i64);
        let b = expr.lit(5i64);

        let mut p = PureEvaluator::default();

        assert_eq!(p.eval(a + b), 12);
        assert_eq!(p.eval(a - b), 2);
        assert_eq!(p.eval(a * b), 35);
    }
}

//! Simple library for handling ASTs for polynomials for ZKP in Rust

use core::ops::{Add, Mul, Neg, Sub};

use bumpalo::Bump;
use starky::evaluation_frame::{StarkEvaluationFrame, StarkFrame};

/// Contains a reference to [`ExprTree`] that is managed by [`ExprBuilder`].
#[derive(Clone, Copy, Debug)]
pub struct Expr<'a, V> {
    expr_tree: &'a ExprTree<'a, V>,
    builder: &'a ExprBuilder,
}

impl<'a, V> Add for Expr<'a, V> {
    type Output = Expr<'a, V>;

    fn add(self, rhs: Self) -> Self::Output { self.builder.add(self, rhs) }
}

impl<'a, V> Add<i64> for Expr<'a, V> {
    type Output = Expr<'a, V>;

    fn add(self, rhs: i64) -> Self::Output {
        let rhs = self.builder.constant(rhs);
        self + rhs
    }
}

impl<'a, V> Add<Expr<'a, V>> for i64 {
    type Output = Expr<'a, V>;

    fn add(self, rhs: Expr<'a, V>) -> Self::Output { rhs + self }
}

impl<'a, V> Neg for Expr<'a, V> {
    type Output = Expr<'a, V>;

    fn neg(self) -> Self::Output { self.builder.neg(self) }
}

impl<'a, V> Sub for Expr<'a, V> {
    type Output = Expr<'a, V>;

    fn sub(self, rhs: Self) -> Self::Output { self.builder.sub(self, rhs) }
}

impl<'a, V> Sub<i64> for Expr<'a, V> {
    type Output = Expr<'a, V>;

    fn sub(self, rhs: i64) -> Self::Output {
        let rhs = self.builder.constant(-rhs);
        self + rhs
    }
}

impl<'a, V> Sub<Expr<'a, V>> for i64 {
    type Output = Expr<'a, V>;

    #[allow(clippy::suspicious_arithmetic_impl)]
    fn sub(self, rhs: Expr<'a, V>) -> Self::Output { self + rhs.builder.neg(rhs) }
}

impl<'a, V> Mul for Expr<'a, V> {
    type Output = Expr<'a, V>;

    fn mul(self, rhs: Self) -> Self::Output { self.builder.mul(self, rhs) }
}

impl<'a, V> Mul<i64> for Expr<'a, V> {
    type Output = Expr<'a, V>;

    fn mul(self, rhs: i64) -> Self::Output {
        let rhs = self.builder.constant(rhs);
        self.builder.mul(self, rhs)
    }
}

impl<'a, V> Mul<Expr<'a, V>> for i64 {
    type Output = Expr<'a, V>;

    fn mul(self, rhs: Expr<'a, V>) -> Self::Output { rhs * self }
}

// TODO: support `|` via multiplication.
// TODO support `&` via distributive law, and integration with constraint
// builder. (a & b) | c == (a | c) & (b | c) == [(a | c), (b | c)]
// where [..] means split into multiple constraints.

/// Expression Builder.  Contains a [`Bump`] memory arena that will allocate and
/// store all the [`ExprTree`]s.
#[derive(Debug, Default)]
pub struct ExprBuilder {
    bump: Bump,
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

    /// Convenience method for creating `UnaOp` nodes
    fn una_op<'a, V>(&'a self, op: UnaOp, expr: Expr<'a, V>) -> Expr<'a, V> {
        let expr = expr.expr_tree;
        let expr_tree = ExprTree::UnaOp { op, expr };

        self.intern(expr_tree)
    }

    /// Create a `Literal` expression
    pub fn lit<V>(&self, value: V) -> Expr<'_, V> { self.intern(ExprTree::Literal { value }) }

    /// Create a `Constant` expression
    pub fn constant<V>(&self, value: i64) -> Expr<'_, V> {
        self.intern(ExprTree::Constant { value })
    }

    /// Create an `Add` expression
    pub fn add<'a, V>(&'a self, left: Expr<'a, V>, right: Expr<'a, V>) -> Expr<'a, V> {
        self.bin_op(BinOp::Add, left, right)
    }

    pub fn neg<'a, V>(&'a self, x: Expr<'a, V>) -> Expr<'a, V> { self.una_op(UnaOp::Neg, x) }

    /// Create a `Sub` expression
    pub fn sub<'a, V>(&'a self, left: Expr<'a, V>, right: Expr<'a, V>) -> Expr<'a, V> {
        self.bin_op(BinOp::Add, left, self.una_op(UnaOp::Neg, right))
    }

    /// Create a `Mul` expression
    pub fn mul<'a, V>(&'a self, left: Expr<'a, V>, right: Expr<'a, V>) -> Expr<'a, V> {
        self.bin_op(BinOp::Mul, left, right)
    }

    pub fn is_binary<'a, V>(&'a self, x: Expr<'a, V>) -> Expr<'a, V>
    where
        V: Copy, {
        x * (1 - x)
    }

    /// Convert from untyped `StarkFrame` to a typed representation.
    ///
    /// We ignore public inputs for now, and leave them as is.
    pub fn to_typed_starkframe<'a, T, U, const N: usize, const N2: usize, View>(
        &'a self,
        vars: &'a StarkFrame<T, U, N, N2>,
    ) -> StarkFrameTyped<View, [U; N2]>
    where
        T: Copy + Clone + Default,
        U: Copy + Clone + Default,
        // We don't actually need the first constraint, but it's useful to make the compiler yell
        // at us, if we mix things up. See the TODO about fixing `StarkEvaluationFrame` to
        // give direct access to its contents.
        View: From<[Expr<'a, T>; N]> + FromIterator<Expr<'a, T>>, {
        // TODO: Fix `StarkEvaluationFrame` to give direct access to its contents, no
        // need for the reference only access.
        StarkFrameTyped {
            local_values: vars
                .get_local_values()
                .iter()
                .map(|&v| self.lit(v))
                .collect(),
            next_values: vars
                .get_next_values()
                .iter()
                .map(|&v| self.lit(v))
                .collect(),
            public_inputs: vars.get_public_inputs().try_into().unwrap(),
        }
    }
}

/// A helper around `StarkFrame` to add types
#[derive(Debug)]
pub struct StarkFrameTyped<T, U> {
    pub local_values: T,
    pub next_values: T,
    pub public_inputs: U,
}

/// Enum for binary operations
#[derive(Debug, Eq, PartialEq, Clone, Copy, Hash)]
pub enum BinOp {
    Add,
    Mul,
}

/// Unary operations
#[derive(Debug, Eq, PartialEq, Clone, Copy, Hash)]
pub enum UnaOp {
    Neg,
}

/// Internal type to represent the expression trees
#[derive(Debug)]
enum ExprTree<'a, V> {
    BinOp {
        op: BinOp,
        left: &'a ExprTree<'a, V>,
        right: &'a ExprTree<'a, V>,
    },
    UnaOp {
        op: UnaOp,
        expr: &'a ExprTree<'a, V>,
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
            ExprTree::UnaOp { op, expr } => {
                let expr = expr.eval_with(evaluator);
                evaluator.una_op(op, expr)
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
    fn una_op(&mut self, op: &UnaOp, expr: V) -> V;
    fn constant(&mut self, value: i64) -> V;
    fn eval(&mut self, expr: Expr<'_, V>) -> V { expr.expr_tree.eval_with(self) }
}

/// Default evaluator for pure values.
#[derive(Default)]
pub struct PureEvaluator {}

impl<V> Evaluator<V> for PureEvaluator
where
    V: Copy + Add<Output = V> + Neg<Output = V> + Mul<Output = V> + From<i64>,
{
    fn bin_op(&mut self, op: &BinOp, left: V, right: V) -> V {
        match op {
            BinOp::Add => left + right,
            BinOp::Mul => left * right,
        }
    }

    fn una_op(&mut self, op: &UnaOp, expr: V) -> V {
        match op {
            UnaOp::Neg => -expr,
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

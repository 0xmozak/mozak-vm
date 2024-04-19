//! Simple library for handling ASTs for polynomials for ZKP in Rust
//!
//! NOTE: so far Expr type _belonged_ to Expr builder.  It could even be
//! considered a singleton type per each expression instance.  However, now we
//! want to relax that requirement, and have some expressions that are not tied
//! to expression builders, so that we can have Default instance for
//! expressions.
//!
//! The current API provided by Expr type are the trait instances, which are
//!
//! - [`Add`]
//!   - [`Expr`] + [`Expr`]
//!   - [`i64`] + [`Expr`]
//!   - [`Expr`] + [`i64`]
//! - [`Sub`]
//!   - [`Expr`] - [`Expr`]
//!   - [`i64`] - [`Expr`]
//!   - [`Expr`] - [`i64`]
//! - [`Mul`]
//!   - [`Expr`] * [`Expr`]
//!   - [`i64`] * [`Expr`]
//!   - [`Expr`] * [`i64`]
//! - [`Neg`]
//!   - (- [`Expr`])
//!
//! Then, the current API for Expr builder was pretty much the ability to inject
//! `V` and i64 into Exprs
//!
//! - (private) intern for internalising ExprTree
//! - (private) binop helper method
//! - (private) unop helper method
//! - lit for V
//! - constant for i64
//! - helper methods
//!   - add
//!   - sub
//!   - mul
//!   - neg
//!
//! There is a private contract between ExprBuilder and Expr, as Expr is just a
//! wrapper around ExprTree provided by ExprBuilder, as builder internally
//! operates on ExprTree.
//!
//! Ideally, we want to provide a basic implementation of ExprBuilder for our
//! end users to extend, but I am not sure how to do that efficiently in Rust
//! yet.
//!
//! I also noticed that sometimes it is easier to extend the Expr type, rather
//! than ExprBuilder.
//!
//! Finally, there is the case of Evaluators, because they do form a contract
//! with internal ExprTree, as they provide the semantics for the operations.
//!
//! # TODO
//!
//! - [ ] TODO: support `|` via multiplication.
//! - [ ] TODO support `&` via distributive law, and integration with constraint
//! builder. (a & b) | c == (a | c) & (b | c) == [(a | c), (b | c)] where [..]
//! means split into multiple constraints.

use core::ops::{Add, Mul, Neg, Sub};

use bumpalo::Bump;
use starky::evaluation_frame::{StarkEvaluationFrame, StarkFrame};

/// Contains a reference to [`ExprTree`] that is managed by [`ExprBuilder`].
#[derive(Clone, Copy, Debug)]
pub enum Expr<'a, V> {
    Basic {
        value: i64,
    },
    Compound {
        expr_tree: &'a ExprTree<'a, V>,
        builder: &'a ExprBuilder,
    },
}

/// Blanket implementation of [`From`] instance for [`Expr`]; as long as a type
/// can be converted to [`i64`], then it can be converted to [`Expr`].
impl<'a, V, W: Into<i64>> From<W> for Expr<'a, V> {
    fn from(value: W) -> Self {
        Expr::Basic { value: value.into() }
    }
}

impl<'a, V> Default for Expr<'a, V> {
    fn default() -> Self { Expr::from(0) }
}

/// Main type to hold expression trees.  Contains Expressions defined on `V`
/// with expression trees that will be alive for at last `'a`.
impl<'a, V> Expr<'a, V> {
    fn bin_op(op: BinOp, lhs: Expr<'a, V>, rhs: Expr<'a, V>) -> Expr<'a, V> {
        match (lhs, rhs) {
            (Expr::Basic { value: left }, Expr::Basic { value: right }) =>
                Expr::from(PureEvaluator::default().bin_op(&op, left, right)),
            (left @ Expr::Compound { builder, .. }, right)
            | (left, right @ Expr::Compound { builder, .. }) => builder.wrap(builder.bin_op(
                op,
                builder.ensure_interned(left),
                builder.ensure_interned(right),
            )),
        }
    }

    fn una_op(op: UnaOp, expr: Expr<'a, V>) -> Expr<'a, V> {
        match expr {
            Expr::Basic { value } => Expr::from(PureEvaluator::default().una_op(&op, value)),
            Expr::Compound { expr_tree, builder } => builder.wrap(builder.una_op(op, expr_tree)),
        }
    }
}

impl<'a, V> Expr<'a, V> {
    pub fn is_binary(self) -> Self
    where
        V: Copy, {
        self * (1 - self)
    }

    /// Reduce a sequence of terms into a single term using powers of `base`.
    pub fn reduce_with_powers<I>(terms: I, base: i64) -> Self
    where
        I: IntoIterator<Item = Self>,
        I::IntoIter: DoubleEndedIterator, {
        terms
            .into_iter()
            .rev()
            .fold(Expr::from(0), |acc, term| acc * base + term)
    }
}

impl<V> Expr<'_, V>
where
    V: Copy,
{
    fn eval_with<E>(&self, evaluator: &mut E) -> V
    where
        E: Evaluator<V> + ?Sized, {
        match self {
            Expr::Basic { value } => evaluator.constant(*value),
            Expr::Compound { expr_tree, .. } => expr_tree.eval_with(evaluator),
        }
    }
}

macro_rules! instances {
    ($op: ident, $fun: ident) => {
        impl<'a, V> $op<Self> for Expr<'a, V> {
            type Output = Self;

            fn $fun(self, rhs: Self) -> Self::Output { Self::bin_op(BinOp::$op, self, rhs) }
        }
        impl<'a, V> $op<i64> for Expr<'a, V> {
            type Output = Expr<'a, V>;

            fn $fun(self, rhs: i64) -> Self::Output {
                Self::bin_op(BinOp::$op, self, Expr::from(rhs))
            }
        }

        impl<'a, V> $op<Expr<'a, V>> for i64 {
            type Output = Expr<'a, V>;

            fn $fun(self, rhs: Expr<'a, V>) -> Self::Output {
                Self::Output::bin_op(BinOp::$op, Expr::from(self), rhs)
            }
        }
    };
}

instances!(Add, add);
instances!(Sub, sub);
instances!(Mul, mul);

impl<'a, V> Neg for Expr<'a, V> {
    type Output = Expr<'a, V>;

    fn neg(self) -> Self::Output { Self::una_op(UnaOp::Neg, self) }
}

/// Expression Builder.  Contains a [`Bump`] memory arena that will allocate and
/// store all the [`ExprTree`]s.
#[derive(Debug, Default)]
pub struct ExprBuilder {
    bump: Bump,
}

impl ExprBuilder {
    /// Internalise an [`ExprTree`] by moving it to memory allocated by the
    /// [`Bump`] arena owned by [`ExprBuilder`].
    fn intern<'a, V>(&'a self, expr_tree: ExprTree<'a, V>) -> &'a ExprTree<'a, V> {
        self.bump.alloc(expr_tree)
    }

    fn ensure_interned<'a, V>(&'a self, expr: Expr<'a, V>) -> &'a ExprTree<'a, V> {
        match expr {
            Expr::Compound { expr_tree, .. } => expr_tree,
            Expr::Basic { value } => self.constant_tree(value),
        }
    }

    /// Wrap [`ExprTree`] reference with an [`Expr`] wrapper
    fn wrap<'a, V>(&'a self, expr_tree: &'a ExprTree<'a, V>) -> Expr<'a, V> {
        Expr::Compound {
            expr_tree,
            builder: self,
        }
    }

    /// Convenience method for creating `BinOp` nodes
    fn bin_op<'a, V>(
        &'a self,
        op: BinOp,
        left: &'a ExprTree<'a, V>,
        right: &'a ExprTree<'a, V>,
    ) -> &'a ExprTree<'a, V> {
        let expr_tree = ExprTree::BinOp { op, left, right };
        self.intern(expr_tree)
    }

    /// Convenience method for creating `UnaOp` nodes
    fn una_op<'a, V>(&'a self, op: UnaOp, expr: &'a ExprTree<'a, V>) -> &'a ExprTree<'a, V> {
        let expr_tree = ExprTree::UnaOp { op, expr };
        self.intern(expr_tree)
    }

    /// Allocate Constant Expression Tree in the Expr Builder
    fn constant_tree<V>(&self, value: i64) -> &ExprTree<'_, V> {
        self.intern(ExprTree::Constant { value })
    }

    fn lit_tree<V>(&self, value: V) -> &ExprTree<'_, V> { self.intern(ExprTree::Literal { value }) }

    /// Create a `Constant` expression
    pub fn constant<V>(&self, value: i64) -> Expr<'_, V> { self.wrap(self.constant_tree(value)) }

    /// Create a `Literal` expression
    pub fn lit<V>(&self, value: V) -> Expr<'_, V> { self.wrap(self.lit_tree(value)) }

    /// Convert from untyped `StarkFrame` to a typed representation.
    ///
    /// We ignore public inputs for now, and leave them as is.
    pub fn to_typed_starkframe<'a, T, U, const N: usize, const N2: usize, View, PublicInputs>(
        &'a self,
        vars: &'a StarkFrame<T, U, N, N2>,
    ) -> StarkFrameTyped<View, PublicInputs>
    where
        T: Copy + Clone + Default + From<U>,
        U: Copy + Clone + Default,
        // We don't actually need the first constraint, but it's useful to make the compiler yell
        // at us, if we mix things up. See the TODO about fixing `StarkEvaluationFrame` to
        // give direct access to its contents.
        View: From<[Expr<'a, T>; N]> + FromIterator<Expr<'a, T>>,
        PublicInputs: From<[Expr<'a, T>; N2]> + FromIterator<Expr<'a, T>>, {
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
            public_inputs: vars
                .get_public_inputs()
                .iter()
                .map(|&v| self.lit(T::from(v)))
                .collect(),
        }
    }
}

/// A helper around `StarkFrame` to add types
#[derive(Debug)]
pub struct StarkFrameTyped<Row, PublicInputs> {
    pub local_values: Row,
    pub next_values: Row,
    pub public_inputs: PublicInputs,
}

/// Enum for binary operations
#[derive(Debug, Eq, PartialEq, Clone, Copy, Hash)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
}

/// Unary operations
#[derive(Debug, Eq, PartialEq, Clone, Copy, Hash)]
pub enum UnaOp {
    Neg,
}

/// Internal type to represent the expression trees
#[derive(Debug)]
pub enum ExprTree<'a, V> {
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
    fn eval(&mut self, expr: Expr<'_, V>) -> V { expr.eval_with(self) }
}

/// Default evaluator for pure values.
#[derive(Default)]
pub struct PureEvaluator {}

impl<V> Evaluator<V> for PureEvaluator
where
    V: Copy + Add<Output = V> + Neg<Output = V> + Mul<Output = V> + Sub<Output = V> + From<i64>,
{
    fn bin_op(&mut self, op: &BinOp, left: V, right: V) -> V {
        match op {
            BinOp::Add => left + right,
            BinOp::Sub => left - right,
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

    #[test]
    fn basic_expressions_work() {
        let expr = ExprBuilder::default();

        let a = expr.lit(7_i64);
        let b = expr.lit(5_i64);

        let c: Expr<'_, i64> = Expr::from(3);

        let mut p = PureEvaluator::default();

        assert_eq!((a + b * c).eval_with(&mut p), 22);
        assert_eq!((a - b * c).eval_with(&mut p), -8);
        assert_eq!((a * b * c).eval_with(&mut p), 105);
    }

    #[test]
    fn basic_expressions_with_no_annotations() {
        let a: Expr<'_, i64> = Expr::from(7_i32);
        let b = Expr::from(5_i64);
        let c = Expr::from(3_u8);

        let mut p = PureEvaluator::default();

        assert_eq!((a + b * c).eval_with(&mut p), 22);
        assert_eq!((a - b * c).eval_with(&mut p), -8);
        assert_eq!((a * b * c).eval_with(&mut p), 105);
    }
}

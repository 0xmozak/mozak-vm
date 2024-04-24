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

use core::iter::Sum;
use core::ops::{Add, Mul, Neg, Sub};
use std::collections::{HashMap, HashSet};

use bumpalo::Bump;
use starky::evaluation_frame::{StarkEvaluationFrame, StarkFrame};

/// Contains a reference to [`ExprTree`] that is managed by [`ExprBuilder`].
#[derive(Clone, Copy, Debug)]
pub enum Expr<'a, V> {
    Basic {
        value: i64,
    },
    Compound {
        expr: CompoundExpr<'a, V>,
        builder: &'a ExprBuilder,
    },
}

impl<'a, V> From<i64> for Expr<'a, V> {
    fn from(value: i64) -> Self { Expr::Basic { value } }
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
                Expr::from(PureEvaluator::default().bin_op(op, left, right)),
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
            Expr::Basic { value } => Expr::from(PureEvaluator::default().una_op(op, value)),
            Expr::Compound { expr, builder } => builder.wrap(builder.una_op(op, expr)),
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

impl<'a, V> Sum for Expr<'a, V> {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self { iter.fold(Expr::from(0), Add::add) }
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
    fn intern<'a, V>(&'a self, expr_tree: ExprTree<'a, V>) -> CompoundExpr<'a, V> {
        self.bump.alloc(expr_tree).into()
    }

    fn ensure_interned<'a, V>(&'a self, expr: Expr<'a, V>) -> CompoundExpr<'a, V> {
        match expr {
            Expr::Compound { expr, .. } => expr,
            Expr::Basic { value } => self.constant_tree(value),
        }
    }

    /// Wrap [`ExprTree`] reference with an [`Expr`] wrapper
    fn wrap<'a, V>(&'a self, expr: CompoundExpr<'a, V>) -> Expr<'a, V> {
        Expr::Compound {
            expr,
            builder: self,
        }
    }

    /// Convenience method for creating `BinOp` nodes
    fn bin_op<'a, V>(
        &'a self,
        op: BinOp,
        left: CompoundExpr<'a, V>,
        right: CompoundExpr<'a, V>,
    ) -> CompoundExpr<'a, V> {
        let expr_tree = ExprTree::BinOp { op, left, right };
        self.intern(expr_tree)
    }

    /// Convenience method for creating `UnaOp` nodes
    fn una_op<'a, V>(&'a self, op: UnaOp, expr: CompoundExpr<'a, V>) -> CompoundExpr<'a, V> {
        let expr_tree = ExprTree::UnaOp { op, expr };
        self.intern(expr_tree)
    }

    /// Allocate Constant Expression Tree in the Expr Builder
    fn constant_tree<V>(&self, value: i64) -> CompoundExpr<'_, V> {
        self.intern(ExprTree::Constant { value })
    }

    fn lit_tree<V>(&self, value: V) -> CompoundExpr<'_, V> {
        self.intern(ExprTree::Literal { value })
    }

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

#[derive(Debug, Clone, Copy)]
pub struct CompoundExpr<'a, V>(&'a ExprTree<'a, V>);

impl<'a, V> From<&'a ExprTree<'a, V>> for CompoundExpr<'a, V> {
    fn from(value: &'a ExprTree<'a, V>) -> Self { CompoundExpr(value) }
}

impl<'a, V> From<&'a mut ExprTree<'a, V>> for CompoundExpr<'a, V> {
    fn from(value: &'a mut ExprTree<'a, V>) -> Self { CompoundExpr(value) }
}

/// Internal type to represent the expression trees
#[derive(Debug)]
pub enum ExprTree<'a, V> {
    BinOp {
        op: BinOp,
        left: CompoundExpr<'a, V>,
        right: CompoundExpr<'a, V>,
    },
    UnaOp {
        op: UnaOp,
        expr: CompoundExpr<'a, V>,
    },
    Literal {
        value: V,
    },
    Constant {
        value: i64,
    },
}

pub trait Evaluator<'a, V: Copy>: Evaluator2<'a, V, V> {}

// TODO(Matthias): we introduce W, so we can have evaluators that go purely for
// side-effects etc. How does bin_op etc work here?
/// Evaluator that can evaluate [`Expr`] to `V`.
pub trait Evaluator2<'a, V, W>
where
    V: Copy, {
    fn bin_op(&mut self, op: BinOp, left: W, right: W) -> W;
    fn una_op(&mut self, op: UnaOp, expr: W) -> W;
    fn constant(&mut self, value: i64) -> W;
    fn literal(&mut self, literal: &V) -> W;
    // We need to open the recursion in expr_tree, if we want to override,
    // ie do only one level.
    fn expr_tree(&mut self, expr_tree: &'a ExprTree<'a, V>) -> W {
        match expr_tree {
            ExprTree::BinOp { op, left, right } => {
                let left = self.compound_expr(*left);
                let right = self.compound_expr(*right);
                self.bin_op(*op, left, right)
            }
            ExprTree::UnaOp { op, expr } => {
                let expr = self.compound_expr(*expr);
                self.una_op(*op, expr)
            }
            ExprTree::Literal { value } => self.literal(value),
            ExprTree::Constant { value } => self.constant(*value),
        }
    }
    // fn expr_tree_open<E: Evaluator<'a, V, >>(&mut self, e: &mut E, expr_tree: &'a
    // ExprTree<'a, V>) -> V {     match expr_tree {
    //         ExprTree::BinOp { op, left, right } => {
    //             let left = e.compound_expr(*left);
    //             let right = e.compound_expr(*right);
    //             self.bin_op(*op, left, right)
    //         }
    //         ExprTree::UnaOp { op, expr } => {
    //             let expr = e.compound_expr(*expr);
    //             self.una_op(*op, expr)
    //         }
    //         ExprTree::Literal { value } => *value,
    //         ExprTree::Constant { value } => self.constant(*value),
    //     }
    // }

    fn compound_expr(&mut self, expr: CompoundExpr<'a, V>) -> W { self.expr_tree(expr.0) }
    fn eval(&mut self, expr: Expr<'a, V>) -> W {
        match expr {
            Expr::Basic { value } => self.constant(value),
            Expr::Compound { expr, builder: _ } => self.compound_expr(expr),
        }
    }
}

/// Default evaluator for pure values.
#[derive(Default)]
pub struct PureEvaluator {}

impl<'a, V> Evaluator2<'a, V, V> for PureEvaluator
where
    V: Copy + Add<Output = V> + Neg<Output = V> + Mul<Output = V> + Sub<Output = V> + From<i64>,
{
    fn literal(&mut self, literal: &V) -> V { *literal }

    fn bin_op(&mut self, op: BinOp, left: V, right: V) -> V {
        match op {
            BinOp::Add => left + right,
            BinOp::Sub => left - right,
            BinOp::Mul => left * right,
        }
    }

    fn una_op(&mut self, op: UnaOp, expr: V) -> V {
        match op {
            UnaOp::Neg => -expr,
        }
    }

    fn constant(&mut self, value: i64) -> V { value.into() }
}

pub struct Numbered<'a, V> {
    // constants: Vec<i64>,
    // constant_cache: HashMap<i64, V>,
    // Hmm, we want an order of evaluation.
    pub eval_order: Vec<*const ExprTree<'a, V>>,
    pub seen: HashSet<*const ExprTree<'a, V>>,
}

// Alas, Rust is too stupid to derive this instance.
impl<'a, V> Default for Numbered<'a, V> {
    fn default() -> Self {
        Numbered {
            eval_order: Vec::default(),
            seen: HashSet::default(),
        }
    }
}

impl<'a, V> Evaluator2<'a, V, ()> for Numbered<'a, V>
where
    V: Copy,
{
    fn literal(&mut self, _literal: &V) {}

    fn una_op(&mut self, _op: UnaOp, _expr: ()) {}

    fn bin_op(&mut self, _op: BinOp, _left: (), _right: ()) {}

    fn constant(&mut self, _value: i64) {}

    fn compound_expr(&mut self, expr: CompoundExpr<'a, V>) {
        // Important: we go down first, and then we assign a number.
        // That way, our values vector is in topological order.
        self.expr_tree(expr.0);
        if self.seen.insert(expr.0) {
            self.eval_order.push(expr.0);
        }
    }
}

#[derive(Default)]
pub struct Cached<'a, V, E> {
    // constants: Vec<i64>,
    constant_cache: HashMap<i64, V>,
    // Hmm, we want an order of evaluation.
    // values: Vec<ExprTree<'a, V>>,
    value_cache: HashMap<*const ExprTree<'a, V>, V>,
    #[allow(dead_code)]
    order: Numbered<'a, V>,
    evaluator: E,
}

impl<'a, V, E> From<E> for Cached<'a, V, E>
where
    E: Evaluator2<'a, V, V>,
    V: Copy,
{
    fn from(value: E) -> Self {
        Cached {
            constant_cache: HashMap::default(),
            value_cache: HashMap::default(),
            order: Numbered::default(),
            evaluator: value,
        }
    }
}

impl<'a, V, E> Evaluator2<'a, V, V> for Cached<'a, V, E>
where
    V: Copy,
    E: Evaluator2<'a, V, V>,
{
    fn literal(&mut self, literal: &V) -> V { *literal }

    fn bin_op(&mut self, op: BinOp, left: V, right: V) -> V {
        self.evaluator.bin_op(op, left, right)
    }

    fn una_op(&mut self, op: UnaOp, expr: V) -> V { self.evaluator.una_op(op, expr) }

    fn constant(&mut self, k: i64) -> V {
        *self
            .constant_cache
            .entry(k)
            .or_insert_with(|| self.evaluator.constant(k))
    }

    // NOTE: We disable clippy warning about map entry becasue it is impossible
    // to implement the following function using entry(k).or_insert_with, due to
    // the closue argument to or_insert_with needing to mutably borrow self for
    // expr_tree, which would be already mutably borrowed by
    // self.value_cache.entry.
    #[allow(clippy::map_entry)]
    fn compound_expr(&mut self, expr: CompoundExpr<'a, V>) -> V {
        // TODO(Matthias): perhaps use 'dynamic programming' approach here, instead of
        // recursive descent? Or we use opne-recursion?
        // That's probably the way to go?
        let expr_tree: &ExprTree<'a, V> = expr.0;
        let k = expr_tree as *const ExprTree<'_, V>;

        // let len = self.value_cache.len();
        // let entry = self.value_cache.entry(k);
        // *entry.or_insert_with(|| {
        //     println!("miss\tCache size: {len}");
        //     let evaluator = &mut self.evaluator;
        //     evaluator.expr_tree_open(&mut self, expr_tree)
        // })

        if !self.value_cache.contains_key(&k) {
            // let evaluator = &mut self.evaluator;
            let v = self.expr_tree(expr_tree);
            self.value_cache.insert(k, v);
            println!("miss\tCache size: {}", self.value_cache.len());
        } else {
            println!("hit\tCache size: {}", self.value_cache.len());
        }

        *self.value_cache.get(&k).unwrap()
    }
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

        assert_eq!(p.eval(a + b * c), 22);
        assert_eq!(p.eval(a - b * c), -8);
        assert_eq!(p.eval(a * b * c), 105);
    }

    #[test]
    fn basic_expressions_with_no_annotations() {
        let a: Expr<'_, i64> = Expr::from(7);
        let b = Expr::from(5);
        let c = Expr::from(3);

        let mut p = PureEvaluator::default();

        assert_eq!(p.eval(a + b * c), 22);
        assert_eq!(p.eval(a - b * c), -8);
        assert_eq!(p.eval(a * b * c), 105);
    }

    #[test]
    fn basic_caching_expressions() {
        let a: Expr<'_, i64> = Expr::from(7);
        let b = Expr::from(5);
        let c = Expr::from(3);

        let mut p = Cached::from(PureEvaluator::default());

        assert_eq!(p.eval(a + b * c), 22);
        assert_eq!(p.eval(a - b * c), -8);
        assert_eq!(p.eval(a * b * c), 105);
    }

    #[test]
    fn avoids_exponential_blowup() {
        let eb = ExprBuilder::default();
        let mut one = eb.lit(1i64);
        // This should timout most modern machines if executed without caching
        for _ in 0..64 {
            one = one * one;
        }

        let mut p = Cached::from(PureEvaluator::default());
        assert_eq!(p.eval(one), 1);
    }
}

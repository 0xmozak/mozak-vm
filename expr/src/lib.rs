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

pub mod ops;

use core::ops::{Add, Mul, Neg, Sub};
use std::collections::HashMap;

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
        View: FromIterator<Expr<'a, T>>,
        PublicInputs: FromIterator<Expr<'a, T>>, {
        // NOTE: Rust needs to know all the intermediate types
        let frame: StarkFrameTyped<Vec<T>, Vec<U>> = StarkFrameTyped::from(vars);
        let frame: StarkFrameTyped<Vec<T>, Vec<T>> = frame.map_public_inputs(|v| T::from(v));
        self.inject_starkframe(frame)
    }

    /// Inject `StarkFrameTypes` into the `ExprBuilder`.
    ///
    /// This function will decompose `StarkFrameTyped` using the `IntoIterator`
    /// instances of `View` and `PublicInputs` and then recompose them back
    /// using `FromIterator` instances of `MappedView` and `MappedPublicInputs`
    /// respectively.
    pub fn inject_starkframe<
        'a,
        T: 'a,
        U: 'a,
        View,
        PublicInputs,
        MappedView,
        MappedPublicInputs,
    >(
        &'a self,
        frame: StarkFrameTyped<View, PublicInputs>,
    ) -> StarkFrameTyped<MappedView, MappedPublicInputs>
    where
        View: IntoIterator<Item = T>,
        PublicInputs: IntoIterator<Item = U>,
        MappedView: FromIterator<Expr<'a, T>>,
        MappedPublicInputs: FromIterator<Expr<'a, U>>, {
        frame
            .map_view(|v| self.lit(v))
            .map_public_inputs(|v| self.lit(v))
    }
}

/// A helper around `StarkFrame` to add types
#[derive(Debug)]
pub struct StarkFrameTyped<View, PublicInputs> {
    pub local_values: View,
    pub next_values: View,
    pub public_inputs: PublicInputs,
}

impl<View, PublicInputs> StarkFrameTyped<View, PublicInputs> {
    pub fn from_values<T, U>(lv: &[T], nv: &[T], pis: &[U]) -> Self
    where
        T: Copy,
        U: Copy,
        View: FromIterator<T>,
        PublicInputs: FromIterator<U>, {
        Self {
            local_values: lv.iter().copied().collect(),
            next_values: nv.iter().copied().collect(),
            public_inputs: pis.iter().copied().collect(),
        }
    }

    pub fn map_view<T, B, F, MappedView>(
        self,
        mut f: F,
    ) -> StarkFrameTyped<MappedView, PublicInputs>
    where
        View: IntoIterator<Item = T>,
        MappedView: FromIterator<B>,
        F: FnMut(T) -> B, {
        StarkFrameTyped {
            local_values: self.local_values.into_iter().map(&mut f).collect(),
            next_values: self.next_values.into_iter().map(f).collect(),
            public_inputs: self.public_inputs,
        }
    }

    pub fn map_public_inputs<U, C, F, MappedPublicInputs>(
        self,
        f: F,
    ) -> StarkFrameTyped<View, MappedPublicInputs>
    where
        PublicInputs: IntoIterator<Item = U>,
        MappedPublicInputs: FromIterator<C>,
        F: FnMut(U) -> C, {
        StarkFrameTyped {
            local_values: self.local_values,
            next_values: self.next_values,
            public_inputs: self.public_inputs.into_iter().map(f).collect(),
        }
    }
}

impl<'a, T, U, const N: usize, const N2: usize, View, PublicInputs>
    From<&'a StarkFrame<T, U, N, N2>> for StarkFrameTyped<View, PublicInputs>
where
    T: Copy + Default,
    U: Copy + Default,
    View: From<[T; N]> + FromIterator<T>,
    PublicInputs: From<[U; N2]> + FromIterator<U>,
{
    fn from(value: &'a StarkFrame<T, U, N, N2>) -> Self {
        Self::from_values(
            value.get_local_values(),
            value.get_next_values(),
            value.get_public_inputs(),
        )
    }
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

/// Evaluator that can evaluate [`Expr`] to `V`.
pub trait Evaluator<'a, V>
where
    V: Copy, {
    fn bin_op(&mut self, op: BinOp, left: V, right: V) -> V;
    fn una_op(&mut self, op: UnaOp, expr: V) -> V;
    fn constant(&mut self, value: i64) -> V;
    fn expr_tree(&mut self, expr_tree: &'a ExprTree<'a, V>) -> V {
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
            ExprTree::Literal { value } => *value,
            ExprTree::Constant { value } => self.constant(*value),
        }
    }
    fn compound_expr(&mut self, expr: CompoundExpr<'a, V>) -> V { self.expr_tree(expr.0) }
    fn eval(&mut self, expr: Expr<'a, V>) -> V {
        match expr {
            Expr::Basic { value } => self.constant(value),
            Expr::Compound { expr, builder: _ } => self.compound_expr(expr),
        }
    }
}

/// Default evaluator for pure values.
pub struct PureEvaluator<P>(pub fn(i64) -> P);

impl<'a, V> Evaluator<'a, V> for PureEvaluator<V>
where
    V: Copy + Add<Output = V> + Neg<Output = V> + Mul<Output = V> + Sub<Output = V>,
{
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

    fn constant(&mut self, value: i64) -> V { (self.0)(value) }
}

impl<V> Default for PureEvaluator<V>
where
    V: Copy + Add<Output = V> + Neg<Output = V> + Mul<Output = V> + Sub<Output = V> + From<i64>,
{
    fn default() -> Self { Self(V::from) }
}

#[derive(Default)]
pub struct Cached<'a, V, E> {
    constant_cache: HashMap<i64, V>,
    value_cache: HashMap<*const ExprTree<'a, V>, V>,
    evaluator: E,
}

impl<'a, V, E> From<E> for Cached<'a, V, E>
where
    E: Evaluator<'a, V>,
    V: Copy,
{
    fn from(value: E) -> Self {
        Cached {
            constant_cache: HashMap::default(),
            value_cache: HashMap::default(),
            evaluator: value,
        }
    }
}

impl<'a, V, E> Evaluator<'a, V> for Cached<'a, V, E>
where
    V: Copy,
    E: Evaluator<'a, V>,
{
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
        let expr_tree = expr.0;
        let k = expr_tree as *const ExprTree<'_, V>;

        if !self.value_cache.contains_key(&k) {
            let v = self.expr_tree(expr_tree);
            self.value_cache.insert(k, v);
        }

        *self.value_cache.get(&k).unwrap()
    }
}

#[derive(Default)]
pub struct Counting<E> {
    count: u64,
    evaluator: E,
}

impl<E> Counting<E> {
    fn inc(&mut self) { self.count += 1; }

    pub fn count(&self) -> u64 { self.count }

    pub fn reset(&mut self) { self.count = 0; }
}

impl<'a, V, E> Evaluator<'a, V> for Counting<E>
where
    E: Evaluator<'a, V>,
    V: Copy,
{
    fn bin_op(&mut self, op: BinOp, left: V, right: V) -> V {
        self.inc();
        self.evaluator.bin_op(op, left, right)
    }

    fn una_op(&mut self, op: UnaOp, expr: V) -> V {
        self.inc();
        self.evaluator.una_op(op, expr)
    }

    fn constant(&mut self, value: i64) -> V {
        self.inc();
        self.evaluator.constant(value)
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
    fn it_works_assign() {
        let expr = ExprBuilder::default();

        let a = expr.lit(7i64);
        let b = expr.lit(5i64);
        let mut c = expr.lit(0i64);

        let mut p = PureEvaluator::default();

        c += a + b;
        assert_eq!(p.eval(c), 12);
        c -= b;
        assert_eq!(p.eval(c), 7);
        c *= b;
        assert_eq!(p.eval(c), 35);
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
    fn count_depth() {
        let eb = ExprBuilder::default();

        let mut c = Counting::<PureEvaluator<_>>::default();
        let mut one = eb.lit(1i64);

        assert_eq!(c.eval(one), 1);
        assert_eq!(c.count(), 0);
        c.reset();

        for _ in 0..10 {
            one = one * one;
        }

        assert_eq!(c.eval(one), 1);
        assert_eq!(c.count(), 1023);
        c.reset();

        let mut c = Cached::from(c);
        assert_eq!(c.eval(one), 1);
        assert_eq!(c.evaluator.count(), 10);
    }

    #[test]
    fn avoids_exponential_blowup() {
        let eb = ExprBuilder::default();
        let mut one = eb.lit(1i64);
        // This should timout most modern machines if executed without caching
        for _ in 0..64 {
            one = one * one;
        }

        let mut p = Cached::<i64, Counting<PureEvaluator<_>>>::default();
        assert_eq!(p.eval(one), 1);
        assert_eq!(p.evaluator.count(), 64);
    }
}

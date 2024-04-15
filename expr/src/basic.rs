use super::*;

#[derive(Debug, Clone, Copy)]
pub enum Expr_<'a, V> {
    Basic(i64),
    Compound(Expr<'a, V>),
}

impl<V> Default for Expr_<'_, V> {
    fn default() -> Self { Self::Basic(0) }
}

impl<'a, V> Expr_<'a, V> {
    /// Reduce a sequence of terms into a single term using powers of `base`.
    pub fn reduce_with_powers<I>(terms: I, base: i64) -> Self
    where
        I: IntoIterator<Item = Self>,
        I::IntoIter: DoubleEndedIterator, {
        let terms = terms.into_iter().rev().peekable();
        let mut sum = Self::Basic(0);
        for term in terms {
            sum = sum * base + term;
        }
        sum
    }
}

impl<'a, V> Add for Expr_<'a, V> {
    type Output = Expr_<'a, V>;

    fn add(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Self::Basic(lhs), Self::Basic(rhs)) => Self::Basic(lhs + rhs),
            (Self::Basic(lhs), Self::Compound(rhs)) => Self::Compound(lhs + rhs),
            (Self::Compound(lhs), Self::Basic(rhs)) => Self::Compound(lhs + rhs),
            (Self::Compound(lhs), Self::Compound(rhs)) => Self::Compound(lhs + rhs),
        }
    }
}

impl<'a, V> Add<i64> for Expr_<'a, V> {
    type Output = Expr_<'a, V>;

    fn add(self, rhs: i64) -> Self::Output { self + Self::Basic(rhs) }
}

impl<'a, V> Add<Expr_<'a, V>> for i64 {
    type Output = Expr_<'a, V>;

    fn add(self, rhs: Expr_<'a, V>) -> Self::Output { Expr_::Basic(self) + rhs }
}

// TODO(Matthias): consider adding Expr_ + Expr variants.

impl<'a, V> Neg for Expr_<'a, V> {
    type Output = Expr_<'a, V>;

    fn neg(self) -> Self::Output {
        match self {
            Self::Basic(val) => Self::Basic(-val),
            Self::Compound(expr) => Self::Compound(-expr),
        }
    }
}

impl<'a, V> Sub for Expr_<'a, V> {
    type Output = Expr_<'a, V>;

    fn sub(self, rhs: Self) -> Self::Output { self + -rhs }
}

impl<'a, V> Sub<i64> for Expr_<'a, V> {
    type Output = Expr_<'a, V>;

    fn sub(self, rhs: i64) -> Self::Output { self + -rhs }
}

impl<'a, V> Sub<Expr_<'a, V>> for i64 {
    type Output = Expr_<'a, V>;

    fn sub(self, rhs: Expr_<'a, V>) -> Self::Output { self + -rhs }
}

// TODO(Matthias): consider using a macro for this?
impl<'a, V> Mul for Expr_<'a, V> {
    type Output = Expr_<'a, V>;

    fn mul(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Self::Basic(lhs), Self::Basic(rhs)) => Self::Basic(lhs * rhs),
            (Self::Basic(lhs), Self::Compound(rhs)) => Self::Compound(lhs * rhs),
            (Self::Compound(lhs), Self::Basic(rhs)) => Self::Compound(lhs * rhs),
            (Self::Compound(lhs), Self::Compound(rhs)) => Self::Compound(lhs * rhs),
        }
    }
}

impl<'a, V> Mul<i64> for Expr_<'a, V> {
    type Output = Expr_<'a, V>;

    fn mul(self, rhs: i64) -> Self::Output { self * Self::Basic(rhs) }
}

impl<'a, V> Mul<Expr_<'a, V>> for i64 {
    type Output = Expr_<'a, V>;

    fn mul(self, rhs: Expr_<'a, V>) -> Self::Output { Expr_::Basic(self) * rhs }
}

impl<V> Expr_<'_, V>
where
    V: Copy,
{
    pub fn eval_with<E>(&self, evaluator: &mut E) -> V
    where
        E: Evaluator<V>,
        E: ?Sized, {
        match self {
            Expr_::Basic(value) => evaluator.constant(*value),
            Expr_::Compound(expr) => expr.expr_tree.eval_with(evaluator),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let expr = ExprBuilder::default();

        let a = Expr_::Compound(expr.lit(7_i64));
        let b = Expr_::Compound(expr.lit(5_i64));
        let c: Expr_<'_, i64> = Expr_::Basic(3);

        let mut p = PureEvaluator::default();

        assert_eq!((a + b * c).eval_with(&mut p), 22);
        assert_eq!((a - b * c).eval_with(&mut p), -8);
        assert_eq!((a * b * c).eval_with(&mut p), 105);
    }
}

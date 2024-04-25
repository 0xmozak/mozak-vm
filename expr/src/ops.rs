//! Trait implementations for traits defined in `std::ops` and
//! `core::iter::Sum`.

use core::iter::Sum;
use std::ops::{Add, Mul, Neg, Sub};

use crate::{BinOp, Expr, UnaOp};

macro_rules! instances {
    ($op: ident, $fun: ident) => {
        impl<'a, V> $op<Self> for Expr<'a, V> {
            type Output = Self;

            fn $fun(self, rhs: Self) -> Self::Output { Self::Output::bin_op(BinOp::$op, self, rhs) }
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

    fn neg(self) -> Self::Output { Self::Output::una_op(UnaOp::Neg, self) }
}

impl<'a, V> Sum<Self> for Expr<'a, V> {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self { iter.fold(Expr::from(0), Add::add) }
}

//! Trait implementations for traits defined in `std::ops` and
//! `core::iter::Sum`.

use core::iter::Sum;
use std::ops::{Add, AddAssign, Mul, MulAssign, Neg, Sub, SubAssign};

use crate::{BinOp, Expr, UnaOp};

macro_rules! binop_instances {
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

        impl<'a, 'b, V> $op<&'b Expr<'a, V>> for Expr<'a, V>
        where
            V: Copy,
        {
            type Output = Expr<'a, V>;

            fn $fun(self, rhs: &'b Expr<'a, V>) -> Self::Output {
                Self::Output::bin_op(BinOp::$op, self, *rhs)
            }
        }
    };
}

binop_instances!(Add, add);
binop_instances!(Sub, sub);
binop_instances!(Mul, mul);

impl<'a, V> Neg for Expr<'a, V> {
    type Output = Expr<'a, V>;

    fn neg(self) -> Self::Output { Self::Output::una_op(UnaOp::Neg, self) }
}

impl<'a, V> Neg for &Expr<'a, V>
where
    V: Copy,
{
    type Output = Expr<'a, V>;

    fn neg(self) -> Self::Output { Self::Output::una_op(UnaOp::Neg, *self) }
}

macro_rules! assign_instances {
    ($trait:ident, $op:ident, $fun: ident) => {
        impl<'a, V> $trait<Self> for Expr<'a, V>
        where
            V: Copy,
        {
            fn $fun(&mut self, rhs: Self) { *self = Self::bin_op(BinOp::$op, *self, rhs) }
        }

        impl<'a, V> $trait<i64> for Expr<'a, V>
        where
            V: Copy,
        {
            fn $fun(&mut self, rhs: i64) {
                *self = Self::bin_op(BinOp::$op, *self, Expr::from(rhs))
            }
        }
    };
}

assign_instances!(AddAssign, Add, add_assign);
assign_instances!(MulAssign, Mul, mul_assign);
assign_instances!(SubAssign, Sub, sub_assign);

impl<'a, V> Sum<Self> for Expr<'a, V> {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self { iter.fold(Expr::from(0), Add::add) }
}

impl<'a, 'b, V> Sum<&'b Expr<'a, V>> for Expr<'a, V>
where
    V: Copy,
{
    fn sum<I: Iterator<Item = &'b Expr<'a, V>>>(iter: I) -> Self {
        iter.fold(Expr::from(0), Add::add)
    }
}

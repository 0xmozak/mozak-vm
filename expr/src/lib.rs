use bumpalo::Bump;

use std::ops::{Add, Div, Mul, Sub};

pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
}

// Handle to an expression tree
#[derive(Clone, Copy)]
pub struct E<'a, V> {
    expr: &'a Expr<'a, V>,
    builder: &'a ExprBuilder,
}

// Convenience method
impl<'a, V> E<'a, V> {
    pub fn eval(self) -> V
    where
        V: Copy,
        V: Add<Output = V>,
        V: Sub<Output = V>,
        V: Mul<Output = V>,
        V: Div<Output = V>,
    {
        self.expr.eval()
    }
}

pub enum Expr<'a, V> {
    BinOp {
        op: BinOp,
        left: &'a Expr<'a, V>,
        right: &'a Expr<'a, V>,
    },
    Literal {
        value: V,
    },
}

impl<V> From<V> for Expr<'_, V> {
    fn from(value: V) -> Self {
        Expr::Literal { value }
    }
}

impl<'a, V> Expr<'a, V> {
    pub fn lit(value: V) -> Self {
        Expr::Literal { value }
    }

    pub fn add(left: &'a Expr<'a, V>, right: &'a Expr<'a, V>) -> Self {
        Expr::BinOp {
            op: BinOp::Add,
            left,
            right,
        }
    }

    pub fn sub(left: &'a Expr<'a, V>, right: &'a Expr<'a, V>) -> Self {
        Expr::BinOp {
            op: BinOp::Sub,
            left,
            right,
        }
    }

    pub fn mul(left: &'a Expr<'a, V>, right: &'a Expr<'a, V>) -> Self {
        Expr::BinOp {
            op: BinOp::Mul,
            left,
            right,
        }
    }

    pub fn div(left: &'a Expr<'a, V>, right: &'a Expr<'a, V>) -> Self {
        Expr::BinOp {
            op: BinOp::Div,
            left,
            right,
        }
    }

    pub fn eval(&self) -> V
    where
        V: Copy,
        V: Add<Output = V>,
        V: Sub<Output = V>,
        V: Mul<Output = V>,
        V: Div<Output = V>,
    {
        match self {
            Expr::Literal { value } => *value,
            Expr::BinOp { op, left, right } => {
                let left = left.eval();
                let right = right.eval();
                match op {
                    BinOp::Add => left + right,
                    BinOp::Sub => left - right,
                    BinOp::Mul => left * right,
                    BinOp::Div => left / right,
                }
            }
        }
    }
}

pub struct ExprBuilder {
    arena: Bump,
}

impl ExprBuilder {
    pub fn new() -> Self {
        Self { arena: Bump::new() }
    }

    pub fn expr<'a, V>(&'a self, expr: &'a mut Expr<'a, V>) -> E<'a, V> {
        // TODO: Consider interning it here
        E {
            expr,
            builder: self,
        }
    }

    pub fn from<'a, V, W>(&'a self, v: V) -> E<'a, W>
    where
        Expr<'a, W>: From<V>,
    {
        let a: Expr<'_, W> = Expr::from(v);
        self.expr(self.arena.alloc(a))
    }

    // Convenience alias for from
    pub fn lit<'a, V>(&'a self, v: V) -> E<'a, V> {
        self.from(v)
    }

    pub fn add<'a, V>(&'a self, left: E<'a, V>, right: E<'a, V>) -> E<'a, V> {
        let left = left.expr;
        let right = right.expr;

        self.expr(self.arena.alloc(Expr::add(left, right)))
    }

    pub fn sub<'a, V>(&'a self, left: E<'a, V>, right: E<'a, V>) -> E<'a, V> {
        let left = left.expr;
        let right = right.expr;

        self.expr(self.arena.alloc(Expr::sub(left, right)))
    }

    pub fn mul<'a, V>(&'a self, left: E<'a, V>, right: E<'a, V>) -> E<'a, V> {
        let left = left.expr;
        let right = right.expr;

        self.expr(self.arena.alloc(Expr::mul(left, right)))
    }

    pub fn div<'a, V>(&'a self, left: E<'a, V>, right: E<'a, V>) -> E<'a, V> {
        let left = left.expr;
        let right = right.expr;

        self.expr(self.arena.alloc(Expr::div(left, right)))
    }
}

impl<'a, V> Add for E<'a, V> {
    type Output = E<'a, V>;

    fn add(self, rhs: Self) -> Self::Output {
        self.builder.add(self, rhs)
    }
}

impl<'a, V> Sub for E<'a, V> {
    type Output = E<'a, V>;

    fn sub(self, rhs: Self) -> Self::Output {
        self.builder.sub(self, rhs)
    }
}

impl<'a, V> Mul for E<'a, V> {
    type Output = E<'a, V>;

    fn mul(self, rhs: Self) -> Self::Output {
        self.builder.mul(self, rhs)
    }
}

impl<'a, V> Div for E<'a, V> {
    type Output = E<'a, V>;

    fn div(self, rhs: Self) -> Self::Output {
        self.builder.div(self, rhs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let expr = ExprBuilder::new();

        let a = expr.from(7);
        let b = expr.from(5);

        assert_eq!(E::eval(a + b), 12);
        assert_eq!(E::eval(a - b), 2);
        assert_eq!(E::eval(a * b), 35);
        assert_eq!(E::eval(a / b), 1);
    }
}

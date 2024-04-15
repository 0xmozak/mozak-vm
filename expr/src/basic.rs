use super::*;

pub enum Expr_<'a, V> {
    Basic(i64),
    Compound(&'a ExprTree<'a, V>),
}

use crate::columns_view::columns_view_impl;

columns_view_impl!(BitShift);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct BitShift<T> {
    pub amount: T,
    pub multiplier: T,
}

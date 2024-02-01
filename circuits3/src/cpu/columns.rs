use crate::columns_view::columns_view_impl;

#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct Add<T> {
    pub op1: [T; 4],
    pub op2: [T; 4],
    pub carry: [T; 3],
    pub out: [T; 4],
}

columns_view_impl!(Add);

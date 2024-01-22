use crate::columns_view::columns_view_impl;

#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct XorColumnsView<T> {
    pub is_execution_row: T,
    pub execution: XorView<T>,
    pub limbs: XorView<[T; 32]>,
}
columns_view_impl!(XorColumnsView);

#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct XorView<T> {
    pub a: T,
    pub b: T,
    pub out: T,
}
columns_view_impl!(XorView);

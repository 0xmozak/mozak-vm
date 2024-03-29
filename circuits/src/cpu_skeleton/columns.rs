use crate::columns_view::{columns_view_impl, make_col_map};

columns_view_impl!(CpuSkeleton);
make_col_map!(CpuSkeleton);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct CpuSkeleton<T> {
    pub clk: T,
    pub pc: T,
    pub is_running: T,
}

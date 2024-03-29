use crate::{columns_view::{columns_view_impl, make_col_map}, linear_combination::Column, stark::mozak_stark::{CpuSkeletonTable, TableWithTypedOutput}};

columns_view_impl!(CpuSkeleton);
make_col_map!(CpuSkeleton);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct CpuSkeleton<T> {
    pub ctl: CpuSkeletonCtl<T>,
    pub is_running: T,
}


columns_view_impl!(CpuSkeletonCtl);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct CpuSkeletonCtl<T> {
    pub clk: T,
    pub pc: T,
}


#[allow(dead_code)]
pub(crate) fn lookup_for_cpu() -> TableWithTypedOutput<CpuSkeleton<Column>> {
    CpuSkeletonTable::new(COL_MAP.ctl, COL_MAP.is_running)
}

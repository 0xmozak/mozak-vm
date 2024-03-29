use crate::columns_view::{columns_view_impl, make_col_map};
use crate::linear_combination::Column;
use crate::stark::mozak_stark::{CpuSkeletonTable, TableWithTypedOutput};

columns_view_impl!(CpuSkeleton);
make_col_map!(CpuSkeleton);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct CpuSkeleton<T> {
    pub clk: T,
    pub pc: T,
    // TODO: whether we can unify is_running and aint_padding.
    pub is_running: T,
    pub aint_padding: T,
}

columns_view_impl!(CpuSkeletonCtl);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct CpuSkeletonCtl<T> {
    pub clk: T,
    pub pc: T,
    pub new_pc: T,
    pub is_running: T,
}

#[allow(dead_code)]
pub(crate) fn lookup_for_cpu() -> TableWithTypedOutput<CpuSkeleton<Column>> {
    CpuSkeletonTable::new(
        CpuSkeletonCtl {
            clk: COL_MAP.clk,
            pc: COL_MAP.pc,
            new_pc: COL_MAP.pc.flip(),
            is_running: COL_MAP.is_running,
        },
        COL_MAP.aint_padding,
    )
}

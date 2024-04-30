use crate::columns_view::{columns_view_impl, make_col_map};
use crate::linear_combination::Column;
use crate::stark::mozak_stark::{SkeletonTable, TableWithTypedOutput};

columns_view_impl!(CpuSkeleton);
make_col_map!(CpuSkeleton);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct CpuSkeleton<T> {
    pub clk: T,
    pub pc: T,
    pub is_running: T,
}

columns_view_impl!(CpuSkeletonCtl);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct CpuSkeletonCtl<T> {
    pub clk: T,
    pub pc: T,
    pub new_pc: T,
    pub will_halt: T,
}

#[allow(dead_code)]
pub(crate) fn lookup_for_cpu() -> TableWithTypedOutput<CpuSkeletonCtl<Column>> {
    SkeletonTable::new(
        CpuSkeletonCtl {
            clk: COL_MAP.clk,
            pc: COL_MAP.pc,
            // The `flip`s here mean that we need at least one row of padding at the end.
            new_pc: COL_MAP.pc.flip(),
            will_halt: !COL_MAP.is_running.flip(),
        },
        COL_MAP.is_running,
    )
}

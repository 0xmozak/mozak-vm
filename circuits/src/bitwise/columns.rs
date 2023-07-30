use itertools::Itertools;
use plonky2::field::types::Field;

use crate::columns_view::{columns_view_impl, make_col_map};
use crate::cross_table_lookup::Column;

#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub(crate) struct BitwiseColumnsView<T> {
    pub(crate) execution: BitwiseExecutionColumnsView<T>,
    pub(crate) op1_limbs: [T; 32],
    pub(crate) op2_limbs: [T; 32],
    pub(crate) res_limbs: [T; 32],
}
columns_view_impl!(BitwiseColumnsView);
make_col_map!(BitwiseColumnsView);

#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub(crate) struct BitwiseExecutionColumnsView<T> {
    pub(crate) is_execution_row: T,
    pub(crate) op1: T,
    pub(crate) op2: T,
    pub(crate) res: T,
}
columns_view_impl!(BitwiseExecutionColumnsView);

/// Columns containing the data which are looked from cpu table into Bitwise
/// stark. [`CpuTable`](crate::cross_table_lookup::CpuTable)
/// [`BitwiseTable`](crate::cross_table_lookup::BitwiseTable).
#[must_use]
pub fn data_for_cpu<F: Field>() -> Vec<Column<F>> {
    Column::singles([MAP.execution.op1, MAP.execution.op2, MAP.execution.res]).collect_vec()
}

/// Column for a binary filter to indicate a lookup from the
/// [`CpuTable`](crate::cross_table_lookup::CpuTable) in the Mozak
/// [`BitwiseTable`](crate::cross_table_lookup::BitwiseTable).
#[must_use]
pub fn filter_for_cpu<F: Field>() -> Column<F> { Column::single(MAP.execution.is_execution_row) }

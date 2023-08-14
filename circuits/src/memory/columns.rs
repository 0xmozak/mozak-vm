use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};

#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct MemoryColumnsView<T> {
    // Indicates if memory is padding.
    pub not_padding: T,

    // Memory address.
    pub mem_addr: T,

    // Clock at memory access.
    pub mem_clk: T,

    // Opcode of memory access.
    pub mem_op: T,

    // Value of memory access.
    pub mem_value: T,

    // Difference between current and previous address.
    pub mem_diff_addr: T,

    // Inverse of the above column. 0 if the above column is 0.
    pub mem_diff_addr_inv: T,

    // Difference between current and previous clock.
    pub mem_diff_clk: T,
}
columns_view_impl!(MemoryColumnsView);
make_col_map!(MemoryColumnsView);

// Total number of columns.
pub const NUM_MEM_COLS: usize = MemoryColumnsView::<()>::NUMBER_OF_COLUMNS;

// // TODO: consider making this as `impl`?
// pub fn rangecheck_looking_mem<F: Field>() -> Vec<Table<F>> {
//     // mem_padding
//     // MAP.mem_padding
//     vec![
//         CpuTable::new(
//             Column::singles([MAP.cpu.dst_value]),
//             Column::single(ops.add),
//         ),
//         CpuTable::new(
//             Column::singles([MAP.cpu.abs_diff]),
//             Column::many([ops.bge, ops.blt]),
//         ),
//     ]
// }

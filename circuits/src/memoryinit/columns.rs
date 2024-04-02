use plonky2::hash::hash_types::RichField;

use crate::columns_view::{columns_view_impl, make_col_map};
use crate::cross_table_lookup::ColumnWithTypedInput;
use crate::linear_combination::Column;
use crate::stark::mozak_stark::TableWithTypedOutput;

columns_view_impl!(MemElement);
/// A Memory Slot that has an address and a value
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct MemElement<T> {
    pub address: T,
    pub value: T,
}

columns_view_impl!(MemoryInit);
make_col_map!(MemoryInit);
/// A Row of Memory generated from both read-only and read-write memory
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct MemoryInit<T> {
    pub element: MemElement<T>,
    /// Filters out instructions that are duplicates, i.e., appear more than
    /// once in the trace.
    pub filter: T,
    /// 1 if this row is a read-write, 0 if this row is read-only
    pub is_writable: T,
}

impl<F: RichField> MemoryInit<F> {
    /// Create a new `MemoryInit` row that is not writable. Useful
    /// for memory traces that are initialized once and never written over.
    #[must_use]
    pub fn new_readonly((addr, value): (u32, u8)) -> Self {
        Self {
            filter: F::ONE,
            is_writable: F::ZERO,
            element: MemElement {
                address: F::from_canonical_u32(addr),
                value: F::from_canonical_u8(value),
            },
        }
    }
}

columns_view_impl!(MemoryInitCtl);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct MemoryInitCtl<T> {
    pub is_writable: T,
    pub address: T,
    pub clk: T,
    pub value: T,
}

/// Columns containing the data which are looked up from the Memory Table
#[must_use]
pub fn lookup_for_memory<T>(new: T) -> TableWithTypedOutput<MemoryInitCtl<Column>>
where
    T: Fn(
        MemoryInitCtl<ColumnWithTypedInput<MemoryInit<i64>>>,
        ColumnWithTypedInput<MemoryInit<i64>>,
    ) -> TableWithTypedOutput<MemoryInitCtl<Column>>, {
    new(
        MemoryInitCtl {
            is_writable: COL_MAP.is_writable,
            address: COL_MAP.element.address,
            clk: ColumnWithTypedInput::constant(1),
            value: COL_MAP.element.value,
        },
        COL_MAP.filter,
    )
}

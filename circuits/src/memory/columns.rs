use core::ops::Add;

use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::RichField;
use plonky2::hash::hashing::PlonkyPermutation;
use plonky2::hash::poseidon2::Poseidon2Permutation;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::plonk::circuit_builder::CircuitBuilder;

use crate::columns_view::{columns_view_impl, make_col_map};
use crate::cross_table_lookup::Column;
use crate::memory_fullword::columns::FullWordMemory;
use crate::memory_halfword::columns::HalfWordMemory;
use crate::memory_io::columns::InputOutputMemory;
use crate::memoryinit::columns::MemoryInit;
use crate::poseidon2_output_bytes::columns::{Poseidon2OutputBytes, BYTES_COUNT};
use crate::poseidon2_sponge::columns::Poseidon2Sponge;
use crate::stark::mozak_stark::{MemoryTable, Table};

/// Represents a row of the memory trace that is transformed from read-only,
/// read-write, halfword and fullword memories
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct Memory<T> {
    /// Indicates if a the memory address is writable.
    pub is_writable: T,

    /// Memory address.
    pub addr: T,

    // Clock at memory access.
    pub clk: T,

    // Operations (one-hot encoded)
    // One of `is_store`, `is_load` or `is_init`(static meminit from ELF) == 1.
    // If none are `1`, it is a padding row
    /// Binary filter column to represent a RISC-V SB operation.
    pub is_store: T,
    /// Binary filter column to represent a RISC-V LB & LBU operation.
    /// Note: Memory table does not concern itself with verifying the
    /// signed nature of the `value` and hence treats `LB` and `LBU`
    /// in the same way.
    pub is_load: T,
    /// Memory Initialisation from ELF (prior to vm execution)
    pub is_init: T,

    /// Value of memory access.
    pub value: T,

    /// Difference between current and previous clock.
    pub diff_clk: T,

    /// Difference between current and previous addresses inversion
    pub diff_addr_inv: T,
}
columns_view_impl!(Memory);
make_col_map!(Memory);

impl<F: RichField> From<&MemoryInit<F>> for Option<Memory<F>> {
    /// All other fields are intentionally set to defaults, and clk is
    /// deliberately set to one, to come after any zero-init rows.
    fn from(row: &MemoryInit<F>) -> Self {
        row.filter.is_one().then(|| Memory {
            is_writable: row.is_writable,
            addr: row.element.address,
            is_init: F::ONE,
            value: row.element.value,
            clk: F::ONE,
            ..Default::default()
        })
    }
}

impl<F: RichField> From<&HalfWordMemory<F>> for Vec<Memory<F>> {
    fn from(val: &HalfWordMemory<F>) -> Self {
        if (val.ops.is_load + val.ops.is_store).is_zero() {
            vec![]
        } else {
            (0..2)
                .map(|i| Memory {
                    clk: val.clk,
                    addr: val.addrs[i],
                    value: val.limbs[i],
                    is_store: val.ops.is_store,
                    is_load: val.ops.is_load,
                    ..Default::default()
                })
                .collect()
        }
    }
}

impl<F: RichField> From<&FullWordMemory<F>> for Vec<Memory<F>> {
    fn from(val: &FullWordMemory<F>) -> Self {
        if (val.ops.is_load + val.ops.is_store).is_zero() {
            vec![]
        } else {
            (0..4)
                .map(|i| Memory {
                    clk: val.clk,
                    addr: val.addrs[i],
                    value: val.limbs[i],
                    is_store: val.ops.is_store,
                    is_load: val.ops.is_load,
                    ..Default::default()
                })
                .collect()
        }
    }
}

impl<F: RichField> From<&Poseidon2Sponge<F>> for Vec<Memory<F>> {
    fn from(value: &Poseidon2Sponge<F>) -> Self {
        if (value.ops.is_permute + value.ops.is_init_permute).is_zero() {
            vec![]
        } else {
            let rate = Poseidon2Permutation::<F>::RATE;
            // each Field element in preimage represents a byte.
            (0..rate)
                .map(|i| Memory {
                    clk: value.clk,
                    addr: value.input_addr
                        + F::from_canonical_u8(u8::try_from(i).expect("i > 255")),
                    is_load: F::ONE,
                    value: value.preimage[i],
                    ..Default::default()
                })
                .collect()
        }
    }
}

impl<F: RichField> From<&Poseidon2OutputBytes<F>> for Vec<Memory<F>> {
    fn from(value: &Poseidon2OutputBytes<F>) -> Self {
        if value.is_executed.is_zero() {
            vec![]
        } else {
            (0..BYTES_COUNT)
                .map(|i| Memory {
                    clk: value.clk,
                    addr: value.output_addr
                        + F::from_canonical_u8(u8::try_from(i).expect(
                            "BYTES_COUNT of poseidon output should be representable by a u8",
                        )),
                    is_store: F::ONE,
                    value: value.output_bytes[i],
                    ..Default::default()
                })
                .collect()
        }
    }
}

impl<F: RichField> From<&InputOutputMemory<F>> for Option<Memory<F>> {
    fn from(val: &InputOutputMemory<F>) -> Self {
        (val.ops.is_memory_store).is_one().then(|| Memory {
            clk: val.clk,
            addr: val.addr,
            value: val.value,
            is_store: val.ops.is_memory_store,
            ..Default::default()
        })
    }
}

impl<T: Clone + Add<Output = T>> Memory<T> {
    pub fn is_executed(&self) -> T {
        let s: Memory<T> = self.clone();
        s.is_store + s.is_load + s.is_init
    }
}

pub fn is_executed_ext_circuit<F: RichField + Extendable<D>, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    values: &Memory<ExtensionTarget<D>>,
) -> ExtensionTarget<D> {
    let tmp = builder.add_extension(values.is_store, values.is_load);
    builder.add_extension(tmp, values.is_init)
}

#[must_use]
pub fn rangecheck_looking() -> Vec<Table> {
    let mem = col_map().map(Column::from);
    vec![
        MemoryTable::new(Column::singles([col_map().addr]), mem.is_executed()),
        MemoryTable::new(Column::singles_diff([col_map().addr]), mem.is_executed()),
        MemoryTable::new(Column::singles([col_map().diff_clk]), mem.is_executed()),
    ]
}

#[must_use]
pub fn rangecheck_u8_looking() -> Vec<Table> {
    let mem = col_map().map(Column::from);
    vec![MemoryTable::new(
        Column::singles([col_map().value]),
        mem.is_executed(),
    )]
}

/// Lookup from the CPU table into Memory
/// stark table.
#[must_use]
pub fn lookup_for_cpu() -> Table {
    let mem = col_map().map(Column::from);
    MemoryTable::new(
        vec![
            mem.clk,
            mem.is_store.clone(),
            mem.is_load.clone(),
            mem.value,
            mem.addr,
        ],
        mem.is_store + mem.is_load,
    )
}

/// Lookup in the `MemoryInit` Table
#[must_use]
pub fn lookup_for_memoryinit() -> Table {
    MemoryTable::new(
        vec![
            Column::single(col_map().is_writable),
            Column::single(col_map().addr),
            Column::single(col_map().clk),
            Column::single(col_map().value),
        ],
        Column::single(col_map().is_init),
    )
}

// TODO(Matthias): consolidate with filter_for_halfword_memory and hook up to CTL!
// Also check the other memory related tables for missing CTL!
/// Columns containing the data which are looked from the CPU table into Memory
/// stark table.
#[must_use]
pub fn data_for_halfword_memory() -> Vec<Column> {
    vec![
        Column::single(col_map().clk),
        Column::single(col_map().addr),
        Column::single(col_map().value),
        Column::single(col_map().is_store),
        Column::single(col_map().is_load),
    ]
}

/// Column for a binary filter to indicate a lookup from the CPU table into
/// Memory stark table.
#[must_use]
pub fn filter_for_halfword_memory() -> Column {
    let mem = col_map().map(Column::from);
    mem.is_store + mem.is_load
}

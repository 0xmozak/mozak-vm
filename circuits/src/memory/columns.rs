use core::ops::Add;

use itertools::izip;
use plonky2::hash::hash_types::RichField;
use plonky2::hash::hashing::PlonkyPermutation;
use plonky2::hash::poseidon2::Poseidon2Permutation;

use crate::columns_view::{columns_view_impl, make_col_map};
use crate::cross_table_lookup::Column;
use crate::memory_halfword::columns::HalfWordMemory;
use crate::memory_zeroinit::columns::MemoryZeroInit;
use crate::memoryinit::columns::{MemoryInit, MemoryInitCtl};
use crate::ops::lw::columns::LoadWord;
use crate::ops::sw::columns::StoreWord;
use crate::poseidon2_output_bytes::columns::{Poseidon2OutputBytes, BYTES_COUNT};
use crate::poseidon2_sponge::columns::Poseidon2Sponge;
use crate::rangecheck::columns::RangeCheckCtl;
use crate::stark::mozak_stark::{MemoryTable, TableWithTypedOutput};
use crate::storage_device::columns::StorageDevice;

/// Represents a row of the memory trace that is transformed from read-only,
/// read-write, halfword and fullword memories
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
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
}
columns_view_impl!(Memory);
make_col_map!(MEM, Memory);

impl<F: RichField> From<&MemoryInit<F>> for Option<Memory<F>> {
    /// All other fields are intentionally set to defaults, and clk is
    /// deliberately set to one, to come after any zero-init rows.
    fn from(row: &MemoryInit<F>) -> Self {
        row.filter.is_one().then(|| Memory {
            is_writable: row.is_writable,
            addr: row.address,
            is_init: F::ONE,
            value: row.value,
            clk: F::ONE,
            ..Default::default()
        })
    }
}

impl<F: RichField> From<&MemoryZeroInit<F>> for Option<Memory<F>> {
    /// Clock `clk` is deliberately set to zero, to come before 'real' init
    /// rows.
    fn from(row: &MemoryZeroInit<F>) -> Self {
        row.filter.is_one().then(|| Memory {
            is_writable: F::ONE,
            addr: row.addr,
            is_init: F::ONE,
            ..Default::default()
        })
    }
}

impl<F: RichField> From<&HalfWordMemory<F>> for Vec<Memory<F>> {
    fn from(val: &HalfWordMemory<F>) -> Self {
        if (val.ops.is_load + val.ops.is_store).is_zero() {
            vec![]
        } else {
            izip!(val.addrs, val.limbs)
                .map(|(addr, value)| Memory {
                    clk: val.clk,
                    addr,
                    value,
                    is_store: val.ops.is_store,
                    is_load: val.ops.is_load,
                    ..Default::default()
                })
                .collect()
        }
    }
}

impl<F: RichField> From<&StoreWord<F>> for Vec<Memory<F>> {
    fn from(val: &StoreWord<F>) -> Self {
        if (val.is_running).is_zero() {
            vec![]
        } else {
            izip!(0.., val.op1_limbs)
                .map(|(i, limb)| Memory {
                    clk: val.clk,
                    addr: val.address + F::from_canonical_u8(i),
                    value: limb,
                    is_store: F::ONE,
                    ..Default::default()
                })
                .collect()
        }
    }
}

impl<F: RichField> From<&LoadWord<F>> for Vec<Memory<F>> {
    fn from(val: &LoadWord<F>) -> Self {
        if (val.is_running).is_zero() {
            vec![]
        } else {
            izip!(0.., val.dst_limbs)
                .map(|(i, value)| Memory {
                    clk: val.clk,
                    addr: val.address + F::from_canonical_u8(i),
                    value,
                    is_load: F::ONE,
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

impl<F: RichField> From<&StorageDevice<F>> for Option<Memory<F>> {
    fn from(val: &StorageDevice<F>) -> Self {
        (val.ops.is_memory_store).is_one().then(|| Memory {
            clk: val.clk,
            addr: val.addr,
            value: val.value,
            is_store: val.ops.is_memory_store,
            ..Default::default()
        })
    }
}

impl<T: Copy + Add<Output = T>> Memory<T> {
    pub fn is_executed(&self) -> T { self.is_store + self.is_load + self.is_init }
}

#[must_use]
pub fn rangecheck_looking() -> Vec<TableWithTypedOutput<RangeCheckCtl<Column>>> {
    vec![
        MemoryTable::new(
            // We treat `is_init` on the next line special, to make sure that inits change the
            // address.
            RangeCheckCtl(MEM.addr.diff() - MEM.is_init.flip()),
            MEM.is_executed(),
        ),
        // Anything but an init has a non-negative clock difference.
        // We augment the clock difference, to make sure that for the same clock cycle the order is
        // as follows: init, load, store, dummy.
        // Specifically, loads should come before stores, so that eg a poseidon ecall that reads
        // and writes to the same memory addresses will do the Right Thing.
        MemoryTable::new(
            // TODO: put augmented_clock function into columns, like for registers.
            RangeCheckCtl((MEM.clk * 4 - MEM.is_store - MEM.is_load * 2 - MEM.is_init * 3).diff()),
            (1 - MEM.is_init).flip(),
        ),
    ]
}

#[must_use]
pub fn rangecheck_u8_looking() -> Vec<TableWithTypedOutput<RangeCheckCtl<Column>>> {
    vec![MemoryTable::new(
        RangeCheckCtl(MEM.value),
        MEM.is_executed(),
    )]
}

columns_view_impl!(MemoryCtl);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct MemoryCtl<T> {
    pub clk: T,
    pub is_store: T,
    pub is_load: T,
    pub addr: T,
    pub value: T,
}

/// Lookup between CPU table and Memory
/// stark table.
#[must_use]
pub fn lookup_for_cpu() -> TableWithTypedOutput<MemoryCtl<Column>> {
    MemoryTable::new(
        MemoryCtl {
            clk: MEM.clk,
            is_store: MEM.is_store,
            is_load: MEM.is_load,
            addr: MEM.addr,
            value: MEM.value,
        },
        MEM.is_store + MEM.is_load,
    )
}

/// Lookup into `MemoryInit` Table
#[must_use]
pub fn lookup_for_memoryinit() -> TableWithTypedOutput<MemoryInitCtl<Column>> {
    MemoryTable::new(
        MemoryInitCtl {
            is_writable: MEM.is_writable,
            address: MEM.addr,
            clk: MEM.clk,
            value: MEM.value,
        },
        MEM.is_init,
    )
}

// TODO(Matthias): add lookups for halfword and fullword memory table.

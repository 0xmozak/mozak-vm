use core::ops::Add;

#[cfg(feature = "enable_poseidon_starks")]
use plonky2::hash::hash_types::NUM_HASH_OUT_ELTS;
use plonky2::hash::poseidon2::WIDTH;

use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};
use crate::cross_table_lookup::ColumnTyped;
#[cfg(feature = "enable_poseidon_starks")]
use crate::linear_combination::Column;
use crate::memory::columns::MemoryCtl;
#[cfg(feature = "enable_poseidon_starks")]
use crate::poseidon2::columns::Poseidon2StateCtl;
#[cfg(feature = "enable_poseidon_starks")]
use crate::poseidon2_output_bytes::columns::Poseidon2OutputBytesCtl;
#[cfg(feature = "enable_poseidon_starks")]
use crate::stark::mozak_stark::{Poseidon2SpongeTable, TableNamed};

#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct Ops<T> {
    pub is_init_permute: T,
    pub is_permute: T,
}

#[repr(C)]
#[derive(Clone, Copy, Default, Eq, PartialEq, Debug)]
pub struct Poseidon2Sponge<T> {
    pub clk: T,
    pub ops: Ops<T>,
    pub input_addr: T,
    pub output_addr: T,
    pub input_len: T,
    pub preimage: [T; WIDTH],
    pub output: [T; WIDTH],
    pub gen_output: T,
}

columns_view_impl!(Poseidon2Sponge);
make_col_map!(Poseidon2Sponge);

pub const NUM_POSEIDON2_SPONGE_COLS: usize = Poseidon2Sponge::<()>::NUMBER_OF_COLUMNS;

impl<T: Clone + Add<Output = T>> Poseidon2Sponge<T> {
    pub fn is_executed(&self) -> T {
        self.ops.is_init_permute.clone() + self.ops.is_permute.clone()
    }
}

columns_view_impl!(Poseidon2SpongeCtl);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct Poseidon2SpongeCtl<T> {
    pub clk: T,
    pub input_addr: T,
    pub input_len: T,
}

#[cfg(feature = "enable_poseidon_starks")]
#[must_use]
pub fn lookup_for_cpu() -> TableNamed<Poseidon2SpongeCtl<Column>> {
    let sponge = COL_MAP;
    Poseidon2SpongeTable::new(
        Poseidon2SpongeCtl {
            clk: sponge.clk,
            input_addr: sponge.input_addr,
            input_len: sponge.input_len,
        },
        COL_MAP.ops.is_init_permute,
    )
}

#[cfg(feature = "enable_poseidon_starks")]
#[must_use]
pub fn lookup_for_poseidon2() -> TableNamed<Poseidon2StateCtl<Column>> {
    let sponge = COL_MAP;
    Poseidon2SpongeTable::new(
        Poseidon2StateCtl {
            input: sponge.preimage,
            output: sponge.output,
        },
        COL_MAP.is_executed(),
    )
    // let mut data = sponge.preimage.to_vec();
    // data.extend(sponge.output.to_vec());
    // data
}

#[cfg(feature = "enable_poseidon_starks")]
#[must_use]
pub fn lookup_for_poseidon2_output_bytes() -> TableNamed<Poseidon2OutputBytesCtl<Column>> {
    let sponge = COL_MAP;
    Poseidon2SpongeTable::new(
        Poseidon2OutputBytesCtl {
            clk: sponge.clk,
            output_addr: sponge.output_addr,
            output_fields: sponge.output[..NUM_HASH_OUT_ELTS].try_into().unwrap(),
        },
        COL_MAP.gen_output,
    )
}

#[cfg(feature = "enable_poseidon_starks")]
#[must_use]
pub fn lookup_for_input_memory(limb_index: u8) -> TableNamed<MemoryCtl<Column>> {
    assert!(limb_index < 8, "limb_index can be 0..7");
    let sponge = COL_MAP;
    let ops = COL_MAP.ops;
    Poseidon2SpongeTable::new(
        MemoryCtl {
            clk: sponge.clk,
            is_store: ColumnTyped::constant(0),
            is_load: ColumnTyped::constant(1),
            value: sponge.preimage[limb_index as usize],
            addr: sponge.input_addr + i64::from(limb_index),
        },
        ops.is_init_permute + ops.is_permute,
    )
}

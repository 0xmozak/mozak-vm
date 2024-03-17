use core::ops::Add;

use plonky2::hash::hash_types::NUM_HASH_OUT_ELTS;
use plonky2::hash::poseidon2::WIDTH;

use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};
use crate::linear_combination_x::ColumnX;
use crate::memory::columns::MemoryCtl;
use crate::poseidon2::columns::Poseidon2StateCtl;
use crate::poseidon2_output_bytes::columns::Poseidon2OutputBytesCtl;

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

type Pos2SpongeCol = ColumnX<Poseidon2Sponge<i64>>;

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

#[must_use]
pub fn data_for_cpu() -> Poseidon2SpongeCtl<Pos2SpongeCol> {
    let sponge = COL_MAP;
    Poseidon2SpongeCtl {
        clk: sponge.clk,
        input_addr: sponge.input_addr,
        input_len: sponge.input_len,
    }
}

#[must_use]
pub fn filter_for_cpu() -> Pos2SpongeCol { COL_MAP.ops.is_init_permute }

// HERE
#[must_use]
pub fn data_for_poseidon2() -> Poseidon2StateCtl<Pos2SpongeCol> {
    let sponge = COL_MAP;
    Poseidon2StateCtl {
        input: sponge.preimage,
        output: sponge.output,
    }
    // let mut data = sponge.preimage.to_vec();
    // data.extend(sponge.output.to_vec());
    // data
}

#[must_use]
pub fn filter_for_poseidon2() -> Pos2SpongeCol { COL_MAP.is_executed() }

#[must_use]
pub fn data_for_poseidon2_output_bytes() -> Poseidon2OutputBytesCtl<Pos2SpongeCol> {
    let sponge = COL_MAP;
    Poseidon2OutputBytesCtl {
        clk: sponge.clk,
        output_addr: sponge.output_addr,
        output_fields: sponge.output[..NUM_HASH_OUT_ELTS].try_into().unwrap(),
    }
}

#[must_use]
pub fn filter_for_poseidon2_output_bytes() -> Pos2SpongeCol { COL_MAP.gen_output }

#[must_use]
pub fn data_for_input_memory(limb_index: u8) -> MemoryCtl<Pos2SpongeCol> {
    assert!(limb_index < 8, "limb_index can be 0..7");
    let sponge = COL_MAP;
    MemoryCtl {
        clk: sponge.clk,
        is_store: ColumnX::constant(0),
        is_load: ColumnX::constant(1),
        value: sponge.preimage[limb_index as usize],
        addr: sponge.input_addr + i64::from(limb_index),
    }
}

#[must_use]
pub fn filter_for_input_memory() -> Pos2SpongeCol {
    let ops = COL_MAP.ops;
    ops.is_init_permute + ops.is_permute
}

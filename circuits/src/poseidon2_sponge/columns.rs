use core::ops::Add;

use plonky2::field::types::Field;
use plonky2::hash::hash_types::{RichField, NUM_HASH_OUT_ELTS};
use plonky2::hash::poseidon2::{Poseidon2, WIDTH};

use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};
use crate::linear_combination::Column;
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
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
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

impl<F: RichField> Default for Poseidon2Sponge<F> {
    fn default() -> Self {
        Self {
            clk: F::default(),
            ops: Ops::<F>::default(),
            input_addr: F::default(),
            input_len: F::default(),
            output_addr: F::default(),
            preimage: [F::default(); WIDTH],
            output: <F as Poseidon2>::poseidon2([F::default(); WIDTH]),
            gen_output: F::default(),
        }
    }
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

#[must_use]
pub fn data_for_cpu<F: Field>() -> Poseidon2SpongeCtl<Column> {
    let sponge = col_map().map(Column::from);
    Poseidon2SpongeCtl {
        clk: sponge.clk,
        input_addr: sponge.input_addr,
        input_len: sponge.input_len,
    }
}

#[must_use]
pub fn filter_for_cpu<F: Field>() -> Column {
    let sponge = col_map().map(Column::from);
    sponge.ops.is_init_permute
}

// HERE
#[must_use]
pub fn data_for_poseidon2<F: Field>() -> Poseidon2StateCtl<Column> {
    let sponge = col_map().map(Column::from);
    Poseidon2StateCtl {
        input: sponge.preimage,
        output: sponge.output,
    }
    // let mut data = sponge.preimage.to_vec();
    // data.extend(sponge.output.to_vec());
    // data
}

#[must_use]
pub fn filter_for_poseidon2<F: Field>() -> Column { col_map().map(Column::from).is_executed() }

#[must_use]
pub fn data_for_poseidon2_output_bytes<F: Field>() -> Poseidon2OutputBytesCtl<Column> {
    let sponge = col_map();
    Poseidon2OutputBytesCtl {
        clk: sponge.clk,
        output_addr: sponge.output_addr,
        output_fields: sponge.output[..NUM_HASH_OUT_ELTS].try_into().unwrap(),
    }
    .map(Column::from)
}

#[must_use]
pub fn filter_for_poseidon2_output_bytes<F: Field>() -> Column {
    col_map().map(Column::from).gen_output
}

#[must_use]
pub fn data_for_input_memory<F: Field>(limb_index: u8) -> MemoryCtl<Column> {
    assert!(limb_index < 8, "limb_index can be 0..7");
    let sponge = col_map().map(Column::from);
    MemoryCtl {
        clk: sponge.clk,
        is_store: Column::constant(F::ZERO),
        is_load: Column::constant(F::ONE),
        value: sponge.preimage[limb_index as usize].clone(),
        addr: sponge.input_addr + F::from_canonical_u8(limb_index),
    }
}

#[must_use]
pub fn filter_for_input_memory<F: Field>() -> Column {
    let row = col_map().map(Column::from);
    row.ops.is_init_permute + row.ops.is_permute
}

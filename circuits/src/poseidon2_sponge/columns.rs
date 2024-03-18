use core::ops::Add;

use plonky2::hash::hash_types::{RichField, NUM_HASH_OUT_ELTS};
use plonky2::hash::poseidon2::{Poseidon2, WIDTH};

use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};
use crate::linear_combination::Column;
use crate::stark::mozak_stark::{Poseidon2SpongeTable, Table};

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

#[must_use]
pub fn data_for_cpu() -> Vec<Column> {
    let sponge = col_map().map(Column::from);
    vec![sponge.clk, sponge.input_addr, sponge.input_len]
}

#[must_use]
pub fn filter_for_cpu() -> Column {
    let sponge = col_map().map(Column::from);
    sponge.ops.is_init_permute
}

#[must_use]
pub fn data_for_poseidon2() -> Vec<Column> {
    let sponge = col_map().map(Column::from);
    let mut data = sponge.preimage.to_vec();
    data.extend(sponge.output.to_vec());
    data
}

#[must_use]
pub fn filter_for_poseidon2() -> Column { col_map().map(Column::from).is_executed() }

#[must_use]
pub fn data_for_poseidon2_output_bytes() -> Vec<Column> {
    let sponge = col_map().map(Column::from);
    let mut data = vec![];
    data.push(sponge.clk);
    data.push(sponge.output_addr);
    let mut outputs = sponge.output.to_vec();
    outputs.truncate(NUM_HASH_OUT_ELTS);
    data.extend(outputs);
    data
}

#[must_use]
pub fn filter_for_poseidon2_output_bytes() -> Column { col_map().map(Column::from).gen_output }

#[must_use]
pub fn lookup_for_input_memory(limb_index: u8) -> Table {
    assert!(limb_index < 8, "limb_index can be 0..7");
    let sponge = col_map().map(Column::from);
    Poseidon2SpongeTable::new(
        vec![
            sponge.clk,
            Column::constant(0),                          // is_store
            Column::constant(1),                          // is_load
            sponge.preimage[limb_index as usize].clone(), // value
            sponge.input_addr + i64::from(limb_index),    // address
        ],
        sponge.ops.is_init_permute + sponge.ops.is_permute,
    )
}

use core::ops::Add;

use plonky2::field::types::Field;
use plonky2::hash::hash_types::RichField;
use plonky2::hash::poseidon2::{Poseidon2, WIDTH};

use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};
use crate::linear_combination::Column;

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
    pub len: T,
    pub preimage: [T; WIDTH],
    pub output: [T; WIDTH],
    pub gen_output: T,
    pub con_input: T,
}

impl<F: RichField> Default for Poseidon2Sponge<F> {
    fn default() -> Self {
        Self {
            clk: F::default(),
            ops: Ops::<F>::default(),
            input_addr: F::default(),
            len: F::default(),
            output_addr: F::default(),
            preimage: [F::default(); WIDTH],
            output: <F as Poseidon2>::poseidon2([F::default(); WIDTH]),
            gen_output: F::default(),
            con_input: F::default(),
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
pub fn data_for_cpu<F: Field>() -> Vec<Column<F>> {
    let sponge = MAP.map(Column::from);
    vec![sponge.clk, sponge.input_addr, sponge.len]
}

#[must_use]
pub fn filter_for_cpu<F: Field>() -> Column<F> {
    let sponge = MAP.map(Column::from);
    sponge.ops.is_init_permute
}

#[must_use]
pub fn data_for_poseidon2<F: Field>() -> Vec<Column<F>> {
    let sponge = MAP.map(Column::from);
    let mut data = sponge.preimage.to_vec();
    data.extend(sponge.output.to_vec());
    data
}

#[must_use]
pub fn filter_for_poseidon2<F: Field>() -> Column<F> { MAP.map(Column::from).is_executed() }

#[must_use]
pub fn data_for_input_memory<F: Field>(limb_index: u8) -> Vec<Column<F>> {
    assert!(limb_index < 8, "limb_index can be 0..7");
    let sponge = MAP.map(Column::from);
    vec![
        sponge.clk,
        Column::constant(F::ZERO),                            // is_store
        Column::constant(F::ONE),                             // is_load
        sponge.preimage[limb_index as usize].clone(),         // value
        sponge.input_addr + F::from_canonical_u8(limb_index), // address
    ]
}

#[must_use]
pub fn filter_for_input_memory<F: Field>() -> Column<F> { MAP.map(Column::from).con_input }

#[must_use]
pub fn data_for_output_memory<F: Field>(limb_index: u8) -> Vec<Column<F>> {
    assert!(limb_index < 8, "limb_index can be 0..7");
    let sponge = MAP.map(Column::from);
    vec![
        sponge.clk,
        Column::constant(F::ONE),                              // is_store
        Column::constant(F::ZERO),                             // is_load
        sponge.output[limb_index as usize].clone(),            // value
        sponge.output_addr + F::from_canonical_u8(limb_index), // address
    ]
}

#[must_use]
pub fn filter_for_output_memory<F: Field>() -> Column<F> { MAP.map(Column::from).gen_output }

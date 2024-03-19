use core::ops::Add;

use plonky2::hash::hash_types::NUM_HASH_OUT_ELTS;
use plonky2::hash::poseidon2::WIDTH;

use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};
#[cfg(feature = "enable_poseidon_starks")]
use crate::linear_combination::Column;
#[cfg(feature = "enable_poseidon_starks")]
use crate::stark::mozak_stark::{Poseidon2SpongeTable, Table};

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
    pub input_addr_padded: T,
}

columns_view_impl!(Poseidon2Sponge);
make_col_map!(Poseidon2Sponge);

pub const NUM_POSEIDON2_SPONGE_COLS: usize = Poseidon2Sponge::<()>::NUMBER_OF_COLUMNS;

impl<T: Clone + Add<Output = T>> Poseidon2Sponge<T> {
    pub fn is_executed(&self) -> T {
        self.ops.is_init_permute.clone() + self.ops.is_permute.clone()
    }
}

#[cfg(feature = "enable_poseidon_starks")]
#[must_use]
pub fn lookup_for_cpu() -> Table {
    let sponge = col_map().map(Column::from);
    Poseidon2SpongeTable::new(
        vec![sponge.clk, sponge.input_addr, sponge.input_len],
        sponge.ops.is_init_permute,
    )
}

#[cfg(feature = "enable_poseidon_starks")]
#[must_use]
pub fn lookup_for_poseidon2() -> Table {
    let sponge = col_map().map(Column::from);
    let mut data = sponge.preimage.to_vec();
    data.extend(sponge.output.to_vec());
    Poseidon2SpongeTable::new(data, col_map().map(Column::from).is_executed())
}

#[cfg(feature = "enable_poseidon_starks")]
#[must_use]
pub fn lookup_for_poseidon2_output_bytes() -> Table {
    let sponge = col_map().map(Column::from);
    let mut data = vec![];
    data.push(sponge.clk);
    data.push(sponge.output_addr);
    let mut outputs = sponge.output.to_vec();
    outputs.truncate(NUM_HASH_OUT_ELTS);
    data.extend(outputs);
    Poseidon2SpongeTable::new(data, col_map().map(Column::from).gen_output)
}

#[cfg(feature = "enable_poseidon_starks")]
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

#[must_use]
pub fn lookup_for_preimage_pack(limb_index: u8) -> Table {
    assert!(limb_index < 8, "limb_index can be 0..7");
    let sponge = col_map().map(Column::from);
    Poseidon2SpongeTable::new(
        vec![
            sponge.clk,
            sponge.preimage[limb_index as usize].clone(), // value
            sponge.input_addr + i64::from(limb_index),    // address
        ],
        sponge.ops.is_init_permute + sponge.ops.is_permute,
    )
}

use plonky2::field::types::Field;
use plonky2::hash::hash_types::{HashOut, RichField};
use plonky2::plonk::config::GenericHashOut;

use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};
use crate::linear_combination::Column;
use crate::memory::columns::MemoryCtl;
use crate::poseidon2_sponge::columns::Poseidon2Sponge;

pub const FIELDS_COUNT: usize = 4;
pub const BYTES_COUNT: usize = 32;

#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct Poseidon2OutputBytes<F> {
    pub is_executed: F,
    pub clk: F,
    pub output_addr: F,
    pub output_fields: [F; FIELDS_COUNT],
    pub output_bytes: [F; BYTES_COUNT],
}

columns_view_impl!(Poseidon2OutputBytes);
make_col_map!(Poseidon2OutputBytes);

pub const NUM_POSEIDON2_OUTPUT_BYTES_COLS: usize = Poseidon2OutputBytes::<()>::NUMBER_OF_COLUMNS;

impl<F: RichField> From<&Poseidon2Sponge<F>> for Vec<Poseidon2OutputBytes<F>> {
    fn from(value: &Poseidon2Sponge<F>) -> Self {
        if value.gen_output.is_one() {
            let output_fields: [F; FIELDS_COUNT] = value.output[..FIELDS_COUNT]
                .try_into()
                .expect("Must have at least 4 Fields");
            let hash_bytes = HashOut::from(output_fields).to_bytes();
            let output_bytes = hash_bytes
                .iter()
                .map(|x| F::from_canonical_u8(*x))
                .collect::<Vec<F>>()
                .try_into()
                .expect("must have 32 bytes");
            return vec![Poseidon2OutputBytes {
                is_executed: F::ONE,
                clk: value.clk,
                output_addr: value.output_addr,
                output_fields,
                output_bytes,
            }];
        }
        vec![]
    }
}

#[must_use]
pub fn data_for_poseidon2_sponge<F: Field>() -> Vec<Column<F>> {
    let data = col_map().map(Column::from);
    let mut data_cols = vec![];
    data_cols.push(data.clk);
    data_cols.push(data.output_addr);
    data_cols.extend(data.output_fields.to_vec());
    data_cols
}

#[must_use]
pub fn filter_for_poseidon2_sponge<F: Field>() -> Column<F> {
    col_map().map(Column::from).is_executed
}

#[must_use]
pub fn data_for_output_memory<F: Field>(limb_index: u8) -> MemoryCtl<Column<F>> {
    assert!(limb_index < 32, "limb_index can be 0..31");
    let data = col_map().map(Column::from);
    MemoryCtl {
        clk: data.clk,
        is_store: Column::constant(F::ONE),
        is_load: Column::constant(F::ZERO),
        value: data.output_bytes[limb_index as usize].clone(),
        addr: data.output_addr + F::from_canonical_u8(limb_index),
    }
}

#[must_use]
pub fn filter_for_output_memory<F: Field>() -> Column<F> { col_map().map(Column::from).is_executed }

use plonky2::field::types::Field;

use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};
use crate::linear_combination::Column;

#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct Ops<T> {
    pub is_init_permute: T,
    pub is_permute: T,
}

#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct Poseidon2Sponge<T> {
    pub clk: T,
    pub ops: Ops<T>,
    pub addr: T,
    pub start_index: T,
    pub preimage: [T; 12],
    pub output: [T; 12],
    pub is_exe: T,
}

columns_view_impl!(Poseidon2Sponge);
make_col_map!(Poseidon2Sponge);

pub const NUM_POSEIDON2_SPONGE_COLS: usize = Poseidon2Sponge::<()>::NUMBER_OF_COLUMNS;

#[must_use]
pub fn data_for_cpu<F: Field>() -> Vec<Column<F>> {
    let sponge = MAP.map(Column::from);
    vec![sponge.clk, sponge.addr, sponge.start_index]
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
pub fn filter_for_poseidon2<F: Field>() -> Column<F> {
    let sponge = MAP.map(Column::from);
    sponge.is_exe
}

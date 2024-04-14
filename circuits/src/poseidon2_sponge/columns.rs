use core::ops::Add;

use mozak_runner::poseidon2::MozakPoseidon2;
use plonky2::hash::hash_types::NUM_HASH_OUT_ELTS;
use plonky2::hash::poseidon2::WIDTH;

use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};
use crate::cross_table_lookup::ColumnWithTypedInput;
use crate::linear_combination::Column;
use crate::memory::columns::MemoryCtl;
use crate::poseidon2::columns::Poseidon2StateCtl;
use crate::poseidon2_output_bytes::columns::Poseidon2OutputBytesCtl;
use crate::poseidon2_preimage_pack::columns::Poseidon2SpongePreimagePackCtl;
use crate::stark::mozak_stark::{Poseidon2SpongeTable, TableWithTypedOutput};

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
make_col_map!(SPONGE, Poseidon2Sponge);

pub const NUM_POSEIDON2_SPONGE_COLS: usize = Poseidon2Sponge::<()>::NUMBER_OF_COLUMNS;

impl<T: Copy + Add<Output = T>> Poseidon2Sponge<T> {
    pub fn is_executed(&self) -> T { self.ops.is_init_permute + self.ops.is_permute }
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
pub fn lookup_for_cpu() -> TableWithTypedOutput<Poseidon2SpongeCtl<Column>> {
    Poseidon2SpongeTable::new(
        Poseidon2SpongeCtl {
            clk: SPONGE.clk,
            input_addr: SPONGE.input_addr,
            input_len: SPONGE.input_len,
        },
        SPONGE.ops.is_init_permute,
    )
}

#[must_use]
pub fn lookup_for_poseidon2() -> TableWithTypedOutput<Poseidon2StateCtl<Column>> {
    Poseidon2SpongeTable::new(
        Poseidon2StateCtl {
            input: SPONGE.preimage,
            output: SPONGE.output,
        },
        SPONGE.is_executed(),
    )
    // let mut data = sponge.preimage.to_vec();
    // data.extend(sponge.output.to_vec());
    // data
}

#[must_use]
pub fn lookup_for_poseidon2_output_bytes() -> TableWithTypedOutput<Poseidon2OutputBytesCtl<Column>>
{
    Poseidon2SpongeTable::new(
        Poseidon2OutputBytesCtl {
            clk: SPONGE.clk,
            output_addr: SPONGE.output_addr,
            output_fields: SPONGE.output[..NUM_HASH_OUT_ELTS].try_into().unwrap(),
        },
        SPONGE.gen_output,
    )
}

#[must_use]
pub fn lookup_for_input_memory(limb_index: u8) -> TableWithTypedOutput<MemoryCtl<Column>> {
    assert!(limb_index < 8, "limb_index can be 0..7");
    Poseidon2SpongeTable::new(
        MemoryCtl {
            clk: SPONGE.clk,
            is_store: ColumnWithTypedInput::constant(0),
            is_load: ColumnWithTypedInput::constant(1),
            value: SPONGE.preimage[limb_index as usize],
            addr: SPONGE.input_addr + i64::from(limb_index),
        },
        SPONGE.ops.is_init_permute + SPONGE.ops.is_permute,
    )
}

#[must_use]
pub fn lookup_for_preimage_pack(
) -> Vec<TableWithTypedOutput<Poseidon2SpongePreimagePackCtl<Column>>> {
    (0..8)
        .zip(SPONGE.preimage)
        .map(|(limb_index, value)| {
            Poseidon2SpongeTable::new(
                Poseidon2SpongePreimagePackCtl {
                    clk: SPONGE.clk,
                    value,
                    byte_addr: SPONGE.input_addr
                        + limb_index
                            * i64::try_from(MozakPoseidon2::DATA_CAPACITY_PER_FIELD_ELEMENT)
                                .expect("Should be < 255"),
                },
                SPONGE.ops.is_init_permute + SPONGE.ops.is_permute,
            )
        })
        .collect()
}

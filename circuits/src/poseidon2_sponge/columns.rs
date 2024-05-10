use core::ops::Add;

use itertools::izip;
use mozak_runner::reg_abi::{REG_A1, REG_A2, REG_A3};
use plonky2::field::goldilocks_field::GoldilocksField;
use plonky2::hash::hash_types::NUM_HASH_OUT_ELTS;
use plonky2::hash::hashing::PlonkyPermutation;
use plonky2::hash::poseidon2::{Poseidon2Permutation, WIDTH};

use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};
use crate::cross_table_lookup::ColumnWithTypedInput;
use crate::linear_combination::Column;
use crate::memory::columns::MemoryCtl;
use crate::poseidon2::columns::Poseidon2StateCtl;
use crate::poseidon2_output_bytes::columns::Poseidon2OutputBytesCtl;
use crate::register::RegisterCtl;
use crate::stark::mozak_stark::{Poseidon2SpongeTable, TableWithTypedOutput};

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

columns_view_impl!(Poseidon2Sponge);
make_col_map!(Poseidon2Sponge);

pub const NUM_POSEIDON2_SPONGE_COLS: usize = Poseidon2Sponge::<()>::NUMBER_OF_COLUMNS;

impl<T: Copy + Add<Output = T>> Poseidon2Sponge<T> {
    pub fn is_executed(&self) -> T { self.ops.is_init_permute + self.ops.is_permute }
}

columns_view_impl!(Poseidon2SpongeCtl);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct Poseidon2SpongeCtl<T> {
    pub clk: T,
}

#[must_use]
pub fn lookup_for_cpu() -> TableWithTypedOutput<Poseidon2SpongeCtl<Column>> {
    Poseidon2SpongeTable::new(
        Poseidon2SpongeCtl { clk: COL_MAP.clk },
        COL_MAP.ops.is_init_permute,
    )
}

#[must_use]
pub fn register_looking() -> Vec<TableWithTypedOutput<RegisterCtl<Column>>> {
    let is_read = ColumnWithTypedInput::constant(1);
    vec![
        Poseidon2SpongeTable::new(
            RegisterCtl {
                clk: COL_MAP.clk,
                op: is_read,
                value: COL_MAP.input_addr,
                addr: ColumnWithTypedInput::constant(REG_A1.into()),
            },
            COL_MAP.ops.is_init_permute,
        ),
        Poseidon2SpongeTable::new(
            RegisterCtl {
                clk: COL_MAP.clk,
                op: is_read,
                value: COL_MAP.input_len,
                addr: ColumnWithTypedInput::constant(REG_A2.into()),
            },
            COL_MAP.ops.is_init_permute,
        ),
        Poseidon2SpongeTable::new(
            RegisterCtl {
                clk: COL_MAP.clk,
                op: is_read,
                value: COL_MAP.output_addr,
                addr: ColumnWithTypedInput::constant(REG_A3.into()),
            },
            COL_MAP.ops.is_init_permute,
        ),
    ]
}

#[must_use]
pub fn lookup_for_poseidon2() -> TableWithTypedOutput<Poseidon2StateCtl<Column>> {
    Poseidon2SpongeTable::new(
        Poseidon2StateCtl {
            input: COL_MAP.preimage,
            output: COL_MAP.output,
        },
        COL_MAP.is_executed(),
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
            clk: COL_MAP.clk,
            output_addr: COL_MAP.output_addr,
            output_fields: COL_MAP.output[..NUM_HASH_OUT_ELTS].try_into().unwrap(),
        },
        COL_MAP.gen_output,
    )
}

pub fn lookup_for_input_memory() -> impl Iterator<Item = TableWithTypedOutput<MemoryCtl<Column>>> {
    izip!(0.., COL_MAP.preimage)
        .take(Poseidon2Permutation::<GoldilocksField>::RATE)
        .map(|(i, value)| {
            Poseidon2SpongeTable::new(
                MemoryCtl {
                    clk: COL_MAP.clk,
                    is_store: ColumnWithTypedInput::constant(0),
                    is_load: ColumnWithTypedInput::constant(1),
                    value,
                    addr: COL_MAP.input_addr + i,
                },
                COL_MAP.ops.is_init_permute + COL_MAP.ops.is_permute,
            )
        })
}

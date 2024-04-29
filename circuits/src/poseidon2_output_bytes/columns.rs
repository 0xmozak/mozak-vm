use itertools::izip;
use plonky2::hash::hash_types::{HashOut, RichField};
use plonky2::plonk::config::GenericHashOut;

use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};
use crate::cross_table_lookup::ColumnWithTypedInput;
use crate::linear_combination::Column;
use crate::memory::columns::MemoryCtl;
use crate::poseidon2_sponge::columns::Poseidon2Sponge;
use crate::stark::mozak_stark::{Poseidon2OutputBytesTable, TableWithTypedOutput};

pub const FIELDS_COUNT: usize = 4;
pub const BYTES_COUNT: usize = 32;

columns_view_impl!(Poseidon2OutputBytes);
make_col_map!(Poseidon2OutputBytes);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct Poseidon2OutputBytes<F> {
    pub is_executed: F,
    pub clk: F,
    pub output_addr: F,
    pub output_fields: [F; FIELDS_COUNT],
    pub output_bytes: [F; BYTES_COUNT],
}

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

columns_view_impl!(Poseidon2OutputBytesCtl);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct Poseidon2OutputBytesCtl<F> {
    pub clk: F,
    pub output_addr: F,
    pub output_fields: [F; FIELDS_COUNT],
}

#[must_use]
pub fn lookup_for_poseidon2_sponge() -> TableWithTypedOutput<Poseidon2OutputBytesCtl<Column>> {
    Poseidon2OutputBytesTable::new(
        Poseidon2OutputBytesCtl {
            clk: COL_MAP.clk,
            output_addr: COL_MAP.output_addr,
            output_fields: COL_MAP.output_fields,
        },
        COL_MAP.is_executed,
    )
}

pub fn lookup_for_output_memory() -> impl Iterator<Item = TableWithTypedOutput<MemoryCtl<Column>>> {
    izip!(0.., COL_MAP.output_bytes).map(move |(limb_index, value)| {
        Poseidon2OutputBytesTable::new(
            MemoryCtl {
                clk: COL_MAP.clk,
                is_store: ColumnWithTypedInput::constant(1),
                is_load: ColumnWithTypedInput::constant(0),
                value,
                addr: COL_MAP.output_addr + limb_index,
            },
            COL_MAP.is_executed,
        )
    })
}

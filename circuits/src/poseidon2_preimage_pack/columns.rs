use itertools::Itertools;
use mozak_runner::poseidon2::MozakPoseidon2;
use plonky2::hash::hash_types::RichField;

use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};
use crate::linear_combination::Column;
use crate::linear_combination_typed::ColumnWithTypedInput;
use crate::memory::columns::MemoryCtl;
use crate::poseidon2::columns::STATE_SIZE;
use crate::poseidon2_sponge::columns::Poseidon2Sponge;
use crate::stark::mozak_stark::{Poseidon2PreimagePackTable, TableWithTypedOutput};

#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct Poseidon2PreimagePack<F> {
    pub clk: F,
    pub byte_addr: F,
    pub fe_addr: F,
    pub bytes: [F; MozakPoseidon2::DATA_CAPACITY_PER_FIELD_ELEMENT],
    pub is_executed: F,
}

columns_view_impl!(Poseidon2PreimagePack);
make_col_map!(PACK, Poseidon2PreimagePack);

pub const NUM_POSEIDON2_PREIMAGE_PACK_COLS: usize = Poseidon2PreimagePack::<()>::NUMBER_OF_COLUMNS;

impl<F: RichField> From<&Poseidon2Sponge<F>> for Vec<Poseidon2PreimagePack<F>> {
    // To make it safe for user to change constants
    #[allow(clippy::assertions_on_constants)]
    fn from(value: &Poseidon2Sponge<F>) -> Self {
        if (value.ops.is_init_permute + value.ops.is_permute).is_zero() {
            vec![]
        } else {
            assert!(
                MozakPoseidon2::FIELD_ELEMENTS_RATE < STATE_SIZE,
                "Packing RATE should be less than STATE_SIZE"
            );
            let preimage: [F; MozakPoseidon2::FIELD_ELEMENTS_RATE] = value.preimage
                [..MozakPoseidon2::FIELD_ELEMENTS_RATE]
                .try_into()
                .expect("Should succeed since preimage can't be empty");
            let mut byte_base_address = value.input_addr_padded;
            let mut fe_base_addr = value.input_addr;
            // For each FE of preimage we have BYTES_COUNT bytes
            preimage
                .iter()
                .map(|fe| {
                    // specific byte address
                    let byte_addr = byte_base_address;
                    // increase by DATA_CAP the byte base address after each iteration
                    byte_base_address += MozakPoseidon2::data_capacity_fe();
                    // specific field-el address
                    let fe_addr = fe_base_addr;
                    // increase by 1 after each iteration
                    fe_base_addr += F::ONE;

                    Poseidon2PreimagePack {
                        clk: value.clk,
                        byte_addr,
                        fe_addr,
                        bytes: <[F; MozakPoseidon2::DATA_CAPACITY_PER_FIELD_ELEMENT]>::try_from(
                            MozakPoseidon2::unpack_to_field_elements(fe),
                        )
                        .unwrap(),
                        is_executed: F::ONE,
                    }
                })
                .collect_vec()
        }
    }
}

columns_view_impl!(Poseidon2SpongePreimagePackCtl);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct Poseidon2SpongePreimagePackCtl<T> {
    pub clk: T,
    pub value: T,
    pub fe_addr: T,
    pub byte_addr: T,
}
#[must_use]
pub fn lookup_for_poseidon2_sponge() -> TableWithTypedOutput<Poseidon2SpongePreimagePackCtl<Column>>
{
    Poseidon2PreimagePackTable::new(
        Poseidon2SpongePreimagePackCtl {
            clk: PACK.clk,
            value: ColumnWithTypedInput::reduce_with_powers(PACK.bytes, 1 << 8),
            fe_addr: PACK.fe_addr,
            byte_addr: PACK.byte_addr,
        },
        PACK.is_executed,
    )
}

#[must_use]
pub fn lookup_for_input_memory() -> Vec<TableWithTypedOutput<MemoryCtl<Column>>> {
    (0..)
        .zip(PACK.bytes)
        .map(|(limb_index, value)| {
            Poseidon2PreimagePackTable::new(
                MemoryCtl {
                    clk: PACK.clk,
                    is_store: ColumnWithTypedInput::constant(0),
                    is_load: ColumnWithTypedInput::constant(1),
                    value,
                    addr: PACK.fe_addr + limb_index,
                },
                PACK.is_executed,
            )
        })
        .collect()
}

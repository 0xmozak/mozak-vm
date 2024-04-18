use itertools::Itertools;
use plonky2::hash::hash_types::RichField;
use poseidon2::mozak_poseidon2;

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
    pub bytes: [F; mozak_poseidon2::DATA_CAPACITY_PER_FIELD_ELEMENT],
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
                mozak_poseidon2::FIELD_ELEMENTS_RATE <= STATE_SIZE,
                "Packing RATE (FIELD_ELEMENTS_RATE) should be less or equal than STATE_SIZE"
            );
            let preimage: [F; mozak_poseidon2::FIELD_ELEMENTS_RATE] = value.preimage
                [..mozak_poseidon2::FIELD_ELEMENTS_RATE]
                .try_into()
                .expect("Should succeed since preimage can't be empty");
            // For each FE of preimage we have PACK_CAP bytes
            preimage
                .iter()
                .enumerate()
                .map(|(i, fe)| Poseidon2PreimagePack {
                    clk: value.clk,
                    byte_addr: value.input_addr
                        + F::from_canonical_usize(i) * mozak_poseidon2::data_capacity_fe::<F>(),
                    bytes: mozak_poseidon2::unpack_to_field_elements(fe),
                    is_executed: F::ONE,
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
    pub byte_addr: T,
}
#[must_use]
pub fn lookup_for_poseidon2_sponge() -> TableWithTypedOutput<Poseidon2SpongePreimagePackCtl<Column>>
{
    Poseidon2PreimagePackTable::new(
        Poseidon2SpongePreimagePackCtl {
            clk: PACK.clk,
            value: ColumnWithTypedInput::reduce_with_powers(PACK.bytes, 1 << 8),
            byte_addr: PACK.byte_addr,
        },
        PACK.is_executed,
    )
}

#[must_use]
pub fn lookup_for_input_memory() -> Vec<TableWithTypedOutput<MemoryCtl<Column>>> {
    (0..)
        .zip(PACK.bytes)
        .map(|(index, value)| {
            Poseidon2PreimagePackTable::new(
                MemoryCtl {
                    clk: PACK.clk,
                    is_store: ColumnWithTypedInput::constant(0),
                    is_load: ColumnWithTypedInput::constant(1),
                    value,
                    addr: PACK.byte_addr + index,
                },
                PACK.is_executed,
            )
        })
        .collect()
}

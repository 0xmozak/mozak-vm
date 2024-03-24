use itertools::Itertools;
use mozak_runner::poseidon2::MozakPoseidon2;
use plonky2::hash::hash_types::RichField;

use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};
use crate::linear_combination::Column;
use crate::poseidon2::columns::STATE_SIZE;
use crate::poseidon2_sponge::columns::Poseidon2Sponge;
use crate::stark::mozak_stark::{Poseidon2PreimagePackTable, Table};

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
make_col_map!(Poseidon2PreimagePack);

pub const NUM_POSEIDON2_PREIMAGE_PACK_COLS: usize = Poseidon2PreimagePack::<()>::NUMBER_OF_COLUMNS;

impl<F: RichField> From<&Poseidon2Sponge<F>> for Vec<Poseidon2PreimagePack<F>> {
    // To make it safe for user to change constants
    #[allow(clippy::assertions_on_constants)]
    fn from(value: &Poseidon2Sponge<F>) -> Self {
        if (value.ops.is_init_permute + value.ops.is_permute).is_one() {
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
            let result = preimage
                .iter()
                .map(|fe| {
                    // Note: assumed `to_be_bytes`, otherwise another side of the array should be
                    // taken
                    // TODO(Roman): consider implementing un-pack function
                    let bytes: Vec<_> = fe.clone().to_canonical_u64().to_be_bytes()
                        [MozakPoseidon2::LEADING_ZEROS..]
                        .iter()
                        .map(|e| F::from_canonical_u8(*e))
                        .collect();
                    // specific byte address
                    let byte_addr = byte_base_address;
                    // increase by DATA_CAP the byte base address after each iteration
                    byte_base_address += F::from_canonical_u64(u64::try_from(MozakPoseidon2::DATA_CAPACITY_PER_FIELD_ELEMENT).expect("Cast from usize to u64 for MozakPoseidon2::BYTES_PER_FIELD_ELEMENT should succeed"));
                    // specific field-el address
                    let fe_addr = fe_base_addr;
                    // increase by 1 after each iteration
                    fe_base_addr += F::ONE;

                    Poseidon2PreimagePack {
                        clk: value.clk,
                        byte_addr,
                        fe_addr,
                        bytes: <[F; MozakPoseidon2::DATA_CAPACITY_PER_FIELD_ELEMENT]>::try_from(
                            bytes,
                        )
                        .unwrap(),
                        is_executed: F::ONE,
                    }
                })
                .collect_vec();
            return result;
        }
        vec![]
    }
}

// To make it safe for user to change constants
#[allow(clippy::assertions_on_constants)]
pub fn transform_poseidon2_sponge<F: RichField>(
    values: Vec<Poseidon2Sponge<F>>,
) -> Vec<Poseidon2PreimagePack<F>> {
    values.iter().map(|value| {
        if (value.ops.is_init_permute + value.ops.is_permute).is_one() {
            assert!(
                MozakPoseidon2::FIELD_ELEMENTS_RATE < STATE_SIZE,
                "Packing RATE should be less than STATE_SIZE"
            );
            let fe_len = MozakPoseidon2::FIELD_ELEMENTS_RATE - usize::try_from(value.fe_padding.to_canonical_u64()).expect("Should succeed");
            let preimage = &value.preimage[..fe_len];
            println!("p-size: {:?}, values.len: {:?}", fe_len, values.len());
            let mut byte_base_address = value.input_addr_padded;
            let mut fe_base_addr = value.input_addr;
            // For each FE of preimage we have BYTES_COUNT bytes
            let result = preimage
                .iter()
                .map(|fe| {
                    // Note: assumed `to_be_bytes`, otherwise another side of the array should be
                    // taken
                    // TODO(Roman): consider implementing un-pack function
                    let bytes: Vec<_> = fe.clone().to_canonical_u64().to_be_bytes()
                        [MozakPoseidon2::LEADING_ZEROS..]
                        .iter()
                        .map(|e| F::from_canonical_u8(*e))
                        .collect();
                    // specific byte address
                    let byte_addr = byte_base_address;
                    // increase by DATA_CAP the byte base address after each iteration
                    byte_base_address += F::from_canonical_u64(u64::try_from(MozakPoseidon2::DATA_CAPACITY_PER_FIELD_ELEMENT).expect("Cast from usize to u64 for MozakPoseidon2::BYTES_PER_FIELD_ELEMENT should succeed"));
                    // specific field-el address
                    let fe_addr = fe_base_addr;
                    // increase by 1 after each iteration
                    fe_base_addr += F::ONE;

                    Poseidon2PreimagePack {
                        clk: value.clk,
                        byte_addr,
                        fe_addr,
                        bytes: <[F; MozakPoseidon2::DATA_CAPACITY_PER_FIELD_ELEMENT]>::try_from(
                            bytes,
                        )
                            .unwrap(),
                        is_executed: F::ONE,
                    }
                })
                .collect_vec();
            println!("p-value: {:?}", value);
            println!("p-result: {:?}", result);
            return result;
        }
        vec![]
    }).flatten().collect()
}
#[must_use]
pub fn lookup_for_poseidon2_sponge() -> Table {
    let data = col_map().map(Column::from);
    Poseidon2PreimagePackTable::new(
        vec![
            data.clk,
            Column::reduce_with_powers(
                // FIXME: Check why does not work just reduce_with_power on &data.bytes
                {
                    let mut r = data.bytes.clone();
                    r.reverse();
                    &r.clone()
                },
                1 << 8,
            ),
            data.fe_addr,
        ],
        col_map().map(Column::from).is_executed,
    )
}
#[must_use]
pub fn lookup_for_input_memory(index: u8) -> Table {
    assert!(
        usize::from(index) < MozakPoseidon2::DATA_CAPACITY_PER_FIELD_ELEMENT,
        "poseidon2-preimage data_for_input_memory: index can be 0..{:?}",
        MozakPoseidon2::DATA_CAPACITY_PER_FIELD_ELEMENT
    );
    let data = col_map().map(Column::from);
    Poseidon2PreimagePackTable::new(
        vec![
            data.clk,
            Column::constant(0),                // is_store
            Column::constant(1),                // is_load
            data.bytes[index as usize].clone(), // value
            data.byte_addr + i64::from(index),  // address
        ],
        col_map().map(Column::from).is_executed,
    )
}

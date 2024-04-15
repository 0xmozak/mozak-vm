#[cfg(not(target_os = "mozakvm"))]
use plonky2::hash::hash_types::RichField;
#[cfg(not(target_os = "mozakvm"))]
use plonky2::hash::hashing::PlonkyPermutation;
#[cfg(not(target_os = "mozakvm"))]
use plonky2::hash::poseidon2::Poseidon2Permutation;
#[allow(clippy::module_name_repetitions)]
pub const DATA_CAPACITY_PER_FIELD_ELEMENT: usize = 7;
pub const DATA_PADDING: usize = DATA_CAPACITY_PER_FIELD_ELEMENT * FIELD_ELEMENTS_RATE;
pub const EMPTY_BYTES: usize = MAX_BYTES_PER_FIELD_ELEMENT - DATA_CAPACITY_PER_FIELD_ELEMENT;
pub const FIELD_ELEMENTS_RATE: usize = 8;
pub const MAX_BYTES_PER_FIELD_ELEMENT: usize = 8;

#[must_use]
#[cfg(not(target_os = "mozakvm"))]
pub fn data_capacity_fe<F: RichField>() -> F {
    F::from_canonical_usize(DATA_CAPACITY_PER_FIELD_ELEMENT)
}

#[must_use]
#[cfg(not(target_os = "mozakvm"))]
pub fn data_padding_fe<F: RichField>() -> F { F::from_canonical_usize(DATA_PADDING) }

/// Byte padding
/// Bit-Padding schema is used to pad input data
/// Case-A - data length % `DATA_PADDING` != 0
/// --> Make first bit of the first padded byte to be 1 - `0b0000_0001`
/// Case-B - data length % `DATA_PADDING` == 0
/// --> Extend padding to next-multiple of `DATA_PADDING` while first bit of
/// the first padded byte will be 1 (same as for Case-A)
#[must_use]
pub fn do_padding(data: &[u8]) -> Vec<u8> {
    let mut padded = data.to_vec();
    padded.push(1);
    padded.resize(padded.len().next_multiple_of(DATA_PADDING), 0);
    padded
}

/// # Panics
///
/// Panics if `Self::DATA_CAPACITY_PER_FIELD_ELEMENT <
/// Self::BYTES_PER_FIELD_ELEMENT`
#[must_use]
// To make it safe for user to change constants
#[allow(clippy::assertions_on_constants)]
#[cfg(not(target_os = "mozakvm"))]
pub fn pack_padded_input<F: RichField>(data: &[u8]) -> Vec<F> {
    assert_eq!(
            Poseidon2Permutation::<F>::RATE,
            FIELD_ELEMENTS_RATE,
            "Poseidon2Permutation::<F>::RATE: {:?} differs from mozak_poseidon2::FIELD_ELEMENTS_RATE: {:?} - is not supported",
            Poseidon2Permutation::<F>::RATE,
            FIELD_ELEMENTS_RATE
        );
    assert!(
        DATA_CAPACITY_PER_FIELD_ELEMENT < MAX_BYTES_PER_FIELD_ELEMENT,
        "For 64 bit field maximum supported packing is 7 bytes"
    );
    assert_eq!(data.len() % DATA_PADDING, 0, "Allow only padded byte-data");
    data.chunks(DATA_CAPACITY_PER_FIELD_ELEMENT)
        .map(pack_to_field_element)
        .collect()
}

/// # Panics
///
/// Panics if `leading-zeros + data` isn't convertable to u64 (length >
/// eight bytes)
#[must_use]
#[cfg(not(target_os = "mozakvm"))]
pub fn pack_to_field_element<F: RichField>(data: &[u8]) -> F {
    // Note: postfix with zeros for LE case
    let mut data_extended_with_zeros: Vec<u8> = data.to_vec();
    data_extended_with_zeros.extend([0_u8; EMPTY_BYTES]);
    assert!(
        data_extended_with_zeros.len() <= 8,
        "data_extended_with_zeros.len {:?} can't be packed to u64::bytes (8)",
        data_extended_with_zeros.len()
    );

    F::from_canonical_u64(u64::from_le_bytes(
        data_extended_with_zeros
            .as_slice()
            .try_into()
            .expect("pack bytes to single u64 should succeed"),
    ))
}

/// # Panics
/// When `Self::DATA_CAPACITY_PER_FIELD_ELEMENT` is larger than the number
/// of bytes in a u64, ie 8.
#[cfg(not(target_os = "mozakvm"))]
pub fn unpack_to_bytes<F: RichField>(fe: &F) -> [u8; DATA_CAPACITY_PER_FIELD_ELEMENT] {
    fe.to_canonical_u64().to_le_bytes()[..DATA_CAPACITY_PER_FIELD_ELEMENT]
        .try_into()
        .unwrap()
}

#[cfg(not(target_os = "mozakvm"))]
pub fn unpack_to_field_elements<F: RichField>(fe: &F) -> [F; DATA_CAPACITY_PER_FIELD_ELEMENT] {
    unpack_to_bytes(fe).map(F::from_canonical_u8)
}

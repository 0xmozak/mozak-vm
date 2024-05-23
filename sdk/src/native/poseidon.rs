// This file contains code snippets used in native execution
use plonky2::field::goldilocks_field::GoldilocksField;
use plonky2::field::types::Field;
use plonky2::hash::poseidon2::Poseidon2Hash as Plonky2Poseidon2Hash;
use plonky2::plonk::config::{GenericHashOut, Hasher};

use crate::common::types::Poseidon2Hash;
use crate::core::constants::RATE;

/// Hashes the input slice to `Poseidon2Hash` after padding.
/// We use the well known "Bit padding scheme".
#[must_use]
pub fn poseidon2_hash_with_pad(input: &[u8]) -> Poseidon2Hash {
    let mut padded_input = input.to_vec();
    padded_input.push(1);

    padded_input.resize(padded_input.len().next_multiple_of(RATE), 0);
    let data_fields: Vec<GoldilocksField> = padded_input
        .iter()
        .map(|x| GoldilocksField::from_canonical_u8(*x))
        .collect();

    Poseidon2Hash(
        Plonky2Poseidon2Hash::hash_no_pad(&data_fields)
            .to_bytes()
            .try_into()
            .expect("Output length does not match to DIGEST_BYTES"),
    )
}

/// Hashes the input slice to `Poseidon2Hash`, assuming
/// the slice length to be of multiple of `RATE`.
/// # Panics
/// If the slice length is not multiple of `RATE`.
/// This is intentional since zkvm's proof system
/// would fail otherwise.
#[allow(unused)]
#[must_use]
pub fn poseidon2_hash_no_pad(input: &[u8]) -> Poseidon2Hash {
    assert!(input.len() % RATE == 0);
    let data_fields: Vec<GoldilocksField> = input
        .iter()
        .map(|x| GoldilocksField::from_canonical_u8(*x))
        .collect();

    Poseidon2Hash(
        Plonky2Poseidon2Hash::hash_no_pad(&data_fields)
            .to_bytes()
            .try_into()
            .expect("Output length does not match to DIGEST_BYTES"),
    )
}

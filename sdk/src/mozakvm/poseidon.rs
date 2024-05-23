#![allow(clippy::module_name_repetitions)]
// This file contains code snippets used in mozakvm execution

use crate::common::types::Poseidon2Hash;
use crate::core::constants::{DIGEST_BYTES, RATE};

/// Hashes the input slice to `Poseidon2Hash` after padding.
/// We use the well known "Bit padding scheme".
#[allow(dead_code)]
pub fn poseidon2_hash_with_pad(input: &[u8]) -> Poseidon2Hash {
    let mut padded_input = input.to_vec();
    padded_input.push(1);

    padded_input.resize(padded_input.len().next_multiple_of(RATE), 0);

    let mut output = [0; DIGEST_BYTES];
    crate::core::ecall::poseidon2(
        padded_input.as_ptr(),
        padded_input.len(),
        output.as_mut_ptr(),
    );
    Poseidon2Hash(output)
}

/// Hashes the input slice to `Poseidon2Hash`, assuming
/// the slice length to be of multiple of `RATE`.
/// # Panics
/// If the slice length is not multiple of `RATE`.
/// This is intentional since zkvm's proof system
/// would fail otherwise.
#[allow(dead_code)]
pub fn poseidon2_hash_no_pad(input: &[u8]) -> Poseidon2Hash {
    assert!(input.len() % RATE == 0);
    let mut output = [0; DIGEST_BYTES];
    crate::core::ecall::poseidon2(input.as_ptr(), input.len(), output.as_mut_ptr());
    Poseidon2Hash(output)
}

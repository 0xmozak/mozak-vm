use std::fs;

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

/// Writes a byte slice to a given file
fn write_to_file(file_path: &str, content: &[u8]) {
    use std::io::Write;
    let path = std::path::Path::new(file_path);
    let mut file = std::fs::File::create(path).unwrap();
    file.write_all(content).unwrap();
}

/// Dumps a copy of `SYSTEM_TAPE` to disk, serialized
/// via `serde_json` as well as in rust debug file format
/// if opted for. Extension of `.tape.json` is used for serialized
/// formed of tape on disk, `.tape.debug` will be used for
/// debug tape on disk.
#[allow(dead_code)]
pub fn dump_system_tape(is_debug_tape_required: bool) {
    fs::create_dir_all("out").unwrap();
    let tape_clone = unsafe {
        crate::common::system::SYSTEM_TAPE.clone() // .clone() removes `Lazy{}`
    };

    if is_debug_tape_required {
        write_to_file("out/tape.debug", &format!("{tape_clone:#?}").into_bytes());
    }

    write_to_file(
        "out/tape.json",
        &serde_json::to_string_pretty(&tape_clone)
            .unwrap()
            .into_bytes(),
    );
}

/// This functions dumps 2 files of the currently running guest program:
///   1. the actual system tape (JSON),
///   2. the debug dump of the system tape,
///
/// These are all dumped in a sub-directory named `out` in the project root. The
/// user must be cautious to not move at least the system tape, as the system
/// tape is used by the CLI in proving and in transaction bundling, and the SDK
/// makes some assumptions about where to find the ELF for proving.
pub fn dump_proving_files() { dump_system_tape(true); }

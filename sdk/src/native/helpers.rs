use std::fs;

// This file contains code snippets used in native execution
use plonky2::field::goldilocks_field::GoldilocksField;
use plonky2::field::types::Field;
use plonky2::hash::poseidon2::Poseidon2Hash as Plonky2Poseidon2Hash;
use plonky2::plonk::config::{GenericHashOut, Hasher};

use crate::common::types::cross_program_call::SelfCallExtendedProgramIdentifier;
use crate::common::types::poseidon2hash::RATE;
use crate::common::types::{Poseidon2Hash, ProgramIdentifier};

/// Represents a stack for call contexts during native execution.
#[derive(Default, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct IdentityStack(Vec<SelfCallExtendedProgramIdentifier>);

impl IdentityStack {
    pub fn add_identity(&mut self, id: SelfCallExtendedProgramIdentifier) { self.0.push(id); }

    pub fn top_identity(&self) -> SelfCallExtendedProgramIdentifier {
        self.0.last().cloned().unwrap_or_default()
    }

    pub fn rm_identity(&mut self) { self.0.truncate(self.0.len().saturating_sub(1)); }
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct GuestProgramTomlCfg {
    bin: Vec<Bin>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub(crate) struct Bin {
    name: String,
    path: String,
}

/// Manually add a `SelfCallExtendedProgramIdentifier` onto `IdentityStack`.
/// Useful when one want to escape automatic management of `IdentityStack`
/// via cross-program-calls sends (ideally temporarily).
/// CAUTION: Manual function for `IdentityStack`, misuse may lead
/// to system tape generation failure.
#[cfg(all(feature = "std", not(target_os = "mozakvm")))]
pub fn add_identity(id: SelfCallExtendedProgramIdentifier) {
    unsafe {
        crate::common::system::SYSTEM_TAPE
            .call_tape
            .identity_stack
            .borrow_mut()
            .add_identity(id);
    }
}

/// Manually remove a `SelfCallExtendedProgramIdentifier` from `IdentityStack`.
/// Useful when one want to escape automatic management of `IdentityStack`
/// via cross-program-calls sends (ideally temporarily).
/// CAUTION: Manual function for `IdentityStack`, misuse may lead
/// to system tape generation failure.
#[cfg(all(feature = "std", not(target_os = "mozakvm")))]
pub fn rm_identity() {
    unsafe {
        crate::common::system::SYSTEM_TAPE
            .call_tape
            .identity_stack
            .borrow_mut()
            .rm_identity();
    }
}

/// Hashes the input slice to `Poseidon2Hash` after padding.
/// We use the well known "Bit padding scheme".
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
pub fn dump_system_tape(file_template: &str, is_debug_tape_required: bool) {
    let tape_clone = unsafe {
        crate::common::system::SYSTEM_TAPE.clone() // .clone() removes `Lazy{}`
    };

    if is_debug_tape_required {
        write_to_file(
            &(file_template.to_string() + ".tape_debug"),
            &format!("{tape_clone:#?}").into_bytes(),
        );
    }

    write_to_file(
        &(file_template.to_string() + ".tape.json"),
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
pub fn dump_proving_files(file_template: &str) {
    fs::create_dir_all("out").unwrap();
    let sys_tape_path = format!("out/{file_template}");
    dump_system_tape(&sys_tape_path, true);
}

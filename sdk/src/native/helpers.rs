use std::fs;
use std::path::PathBuf;

// This file contains code snippets used in native execution
use plonky2::field::goldilocks_field::GoldilocksField;
use plonky2::field::types::Field;
use plonky2::hash::poseidon2::Poseidon2Hash as Plonky2Poseidon2Hash;
use plonky2::plonk::config::{GenericHashOut, Hasher};

use crate::common::types::poseidon2hash::RATE;
use crate::common::types::{Poseidon2Hash, ProgramIdentifier};

/// Represents a stack for call contexts during native execution.
#[derive(Default, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct IdentityStack(Vec<ProgramIdentifier>);

impl IdentityStack {
    pub fn add_identity(&mut self, id: ProgramIdentifier) { self.0.push(id); }

    pub fn top_identity(&self) -> ProgramIdentifier { self.0.last().copied().unwrap_or_default() }

    pub fn rm_identity(&mut self) { self.0.truncate(self.0.len().saturating_sub(1)); }
}

/// A bundle that declares the elf and system tape to be proven together.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ProofBundle {
    pub self_prog_id: String,
    pub elf_filename: String,
    pub system_tape_filepath: PathBuf,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct GuestProgramTomlCfg {
    bin: Vec<Bin>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct Bin {
    name: String,
    path: String,
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

#[allow(dead_code)]
pub fn dump_proving_files(file_template: String, self_prog_id: ProgramIdentifier) {
    dump_system_tape(&file_template.clone(), true);
    let bin_filename = file_template.clone() + ".tape.json";

    let toml_str = fs::read_to_string("Cargo.toml").unwrap();
    let curr_dir = std::env::current_dir().unwrap();
    let toml: GuestProgramTomlCfg = toml::from_str(&toml_str).unwrap();

    let bin_filepath_absolute = curr_dir.join(bin_filename);

    let mozak_bin = toml
        .bin
        .into_iter()
        .find(|b| b.path.contains("_mozak"))
        .expect(
            "Guest program does not have a mozakvm bin with path
 *_mozak.rs declared",
        );

    let elf_filename = mozak_bin.name;
    let bundle = ProofBundle {
        self_prog_id: format!("{self_prog_id:?}"),
        elf_filename,
        system_tape_filepath: bin_filepath_absolute,
    };
    println!("[BNDLDMP] Bundle dump: {bundle:?}");

    let bundle_filename = file_template + "_bundle.json";
    let bundle_json = serde_json::to_string_pretty(&bundle).unwrap();
    write_to_file(&bundle_filename, bundle_json.as_bytes());
}

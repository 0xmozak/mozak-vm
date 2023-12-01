// #![feature(restricted_std)]
mod core_logic;

use std::fs::File;
use std::io::{Read, Write};

use rkyv::{Deserialize, Infallible};
use rs_merkle::algorithms::Sha256;
use rs_merkle::{Hasher, MerkleProof, MerkleTree};

use crate::core_logic::{
    from_tape_function_id, from_tape_rawbuf, from_tape_serialized, to_tape_function_id,
    to_tape_rawbuf, to_tape_serialized, verify_merkle_proof, MerkleRootType, ProofData,
};

/// Generates a merkle tree based on the leaf values given
/// Returns merkle root and ProofData. Uses `SHA256` hasher
/// TODO: Change this to poseidon
fn generate_merkle(
    leaf_values: Vec<u32>,
    indices_to_prove: Vec<u32>,
) -> (
    MerkleRootType, // To be used in "public" tape
    ProofData,      // To be used in "private" tape
) {
    let hashed_leaves: Vec<[u8; 32]> = leaf_values
        .iter()
        .map(|x| Sha256::hash(x.to_le_bytes().as_ref()))
        .collect();

    let merkle_tree = MerkleTree::<Sha256>::from_leaves(&hashed_leaves);

    let leaves_hashes = indices_to_prove
        .iter()
        .map(|idx| hashed_leaves[*idx as usize])
        .collect();

    return (merkle_tree.root().unwrap(), ProofData {
        indices_to_prove: indices_to_prove.clone(),
        leaf_hashes: leaves_hashes,
        proof_bytes: merkle_tree
            .proof(
                indices_to_prove
                    .iter()
                    .map(|x| *x as usize)
                    .collect::<Vec<usize>>()
                    .as_ref(),
            )
            .to_bytes(),
        leaves_len: leaf_values.len() as u32,
    });
}

/// Serializes the merkle root and proof data to disk
/// `files` denote tapes in order `public`, `private`.
/// Uses `rkyv` serialization.
fn serialize_to_disk(files: [&str; 2], merkle_root: [u8; 32], proof_data: &ProofData) {
    println!("Serializing and dumping to disk");

    let mut tapes = get_tapes_native(false, files);

    // Public tape
    to_tape_function_id(&mut tapes[0], 0);
    to_tape_rawbuf(&mut tapes[0], &merkle_root);

    // Private tape
    to_tape_serialized::<ProofData, 256>(&mut tapes[1], proof_data);

    println!("Serializing and dumping to disk [done]");
}

fn get_tapes_native(is_read: bool, files: [&str; 2]) -> Vec<File> {
    let mut new_oo = std::fs::OpenOptions::new();

    let open_options = match is_read {
        true => new_oo.read(true).write(false),
        false => new_oo.append(true).create(true),
    };
    files
        .iter()
        .map(|x| open_options.open(x).expect("cannot open tape"))
        .collect()
}

fn deserialize_from_disk(
    files: [&str; 2],
    expected_merkle_root: [u8; 32],
    expected_testdata: ProofData,
) -> ([u8; 32], ProofData) {
    println!("Reading, deserializing and verifying buffers from disk");

    let mut tapes = get_tapes_native(true, files);

    // Public tape
    let fn_id: u8 = from_tape_function_id(&mut tapes[0]);
    let merkle_root: MerkleRootType = from_tape_rawbuf::<32>(&mut tapes[0]);

    // Private tape
    let proof_data = from_tape_serialized::<ProofData, 256>(&mut tapes[1]);

    println!("Reading, deserializing and verifying buffers from disk [done]");

    (merkle_root, proof_data)
}

fn main() {
    println!("Running merkleproof-trustedroot-native");

    let files = ["public_input.tape", "private_input.tape"];
    let (merkle_root, proof_data) =
        generate_merkle(vec![21, 32, 101, 201, 1, 2, 3, 90], vec![3, 4]);

    serialize_to_disk(files, merkle_root, &proof_data);

    let (merkle_root, proof_data) = deserialize_from_disk(files, merkle_root, proof_data);

    verify_merkle_proof(merkle_root, proof_data);

    println!("all done!")
}

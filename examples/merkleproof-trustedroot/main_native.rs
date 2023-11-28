// #![feature(restricted_std)]
mod core_logic;

use std::fs::File;
use std::io::{Read, Write};

use rkyv::Deserialize;
use rs_merkle::algorithms::Sha256;
use rs_merkle::{Hasher, MerkleProof, MerkleTree};

use crate::core_logic::{
    to_tape_function_id, to_tape_rawbuf, to_tape_serialized, verify_merkle_proof, MerkleRootType,
    ProofData,
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

    let mut tapes: Vec<std::fs::File> = files
        .iter()
        .map(|x| {
            std::fs::OpenOptions::new()
                .read(true)
                .write(false)
                .open(x)
                .expect("cannot open tape")
        })
        .collect();

    // Read public tape (33 bytes)
    let mut function_id_buffer = [0u8; 1];
    let mut merkle_root_buffer = [0u8; 32];

    // Function ID (u8)
    tapes[0]
        .read(&mut function_id_buffer)
        .expect("(public) read failed for function ID");
    assert_eq!(function_id_buffer[0], 0, "function ID mismatched");

    // Merkle state root ([u8; 32])
    tapes[0]
        .read(&mut merkle_root_buffer)
        .expect("(public) read failed for merkle root");
    assert_eq!(
        merkle_root_buffer, expected_merkle_root,
        "merkle root mismatched"
    );

    // Read private tape (variable)
    let mut length_prefix = [0u8; 4];
    // Length prefix (u32)
    tapes[1]
        .read(&mut length_prefix)
        .expect("(private) read failed for length prefix");

    let length_prefix = u32::from_le_bytes(length_prefix);

    let mut testdata_buf = Vec::with_capacity(length_prefix as usize);
    testdata_buf.resize(length_prefix as usize, 0);
    tapes[1]
        .read(&mut testdata_buf[0..(length_prefix as usize)])
        .expect("(private) read failed for merkle proof data");

    let archived = unsafe { rkyv::archived_root::<ProofData>(&testdata_buf) };
    let deserialized_testdata: ProofData = archived.deserialize(&mut rkyv::Infallible).unwrap();

    assert_eq!(
        deserialized_testdata, expected_testdata,
        "testdata mismatch"
    );

    println!("Reading, deserializing and verifying buffers from disk [done]");

    (merkle_root_buffer, deserialized_testdata)
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

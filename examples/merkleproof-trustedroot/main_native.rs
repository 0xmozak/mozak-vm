// #![feature(restricted_std)]
mod core_logic;
use std::fs::File;

use mozak_sdk::io::{
    from_tape_deserialized, from_tape_function_id, from_tape_rawbuf, get_tapes_native,
    to_tape_function_id, to_tape_rawbuf, to_tape_serialized,
};
use rs_merkle::algorithms::Sha256;
use rs_merkle::{Hasher, MerkleTree};
use simple_logger::{set_up_color_terminal, SimpleLogger};

use crate::core_logic::{verify_merkle_proof, MerkleRootType, ProofData};

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
    log::info!("Serializing and dumping to disk: {:#?}", files);

    let mut tapes = get_tapes_native(false, files);

    // Public tape
    to_tape_function_id(&mut tapes[0], 0);

    log::info!("Merkle root: 0x{}", hex::encode(&merkle_root));
    to_tape_rawbuf(&mut tapes[0], &merkle_root);

    // Private tape
    log::info!("Proof Data: {:#?}", proof_data);
    to_tape_serialized::<File, ProofData, 256>(&mut tapes[1], proof_data);

    log::info!("Serializing and dumping to disk: [done]");
}

fn deserialize_from_disk(
    files: [&str; 2],
    expected_merkle_root: MerkleRootType,
    expected_proofdata: ProofData,
) -> (MerkleRootType, ProofData) {
    log::info!(
        "Reading, deserializing and verifying buffers from disk: {:#?}",
        files
    );

    let mut tapes = get_tapes_native(true, files);

    // Public tape
    let fn_id: u8 = from_tape_function_id(&mut tapes[0]);
    assert_eq!(fn_id, 0, "function ID not 0");

    let merkle_root: MerkleRootType = from_tape_rawbuf::<File, 32>(&mut tapes[0]);
    assert_eq!(merkle_root, expected_merkle_root, "unexpected merkle root");

    // Private tape
    let proof_data = from_tape_deserialized::<File, ProofData, 256>(&mut tapes[1]);
    assert_eq!(proof_data, expected_proofdata, "unexpected proof data");

    log::info!("Reading, deserializing and verifying buffers from disk:[done]");

    (merkle_root, proof_data)
}

fn main() {
    SimpleLogger::new().init().unwrap();
    set_up_color_terminal();

    log::info!("Running merkleproof-trustedroot-native");

    let files = ["public_input.tape", "private_input.tape"];
    let (merkle_root, proof_data) =
        generate_merkle(vec![21, 32, 101, 201, 1, 2, 3, 90], vec![3, 4]);

    serialize_to_disk(files, merkle_root, &proof_data);

    let (merkle_root, proof_data) = deserialize_from_disk(files, merkle_root, proof_data);

    verify_merkle_proof(merkle_root, proof_data);

    log::info!("Generated tapes and verified proof, all done!");
}

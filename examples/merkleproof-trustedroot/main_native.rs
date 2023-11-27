// #![feature(restricted_std)]
mod core_logic;

use std::io::Write;

use rs_merkle::algorithms::Sha256;
use rs_merkle::{Hasher, MerkleTree};

use crate::core_logic::TestData;

fn generate_merkle(leaf_values: Vec<u32>, indices_to_prove: Vec<u32>) -> ([u8; 32], TestData) {
    let hashed_leaves: Vec<[u8; 32]> = leaf_values
        .iter()
        .map(|x| Sha256::hash(x.to_le_bytes().as_ref()))
        .collect();

    let merkle_tree = MerkleTree::<Sha256>::from_leaves(&hashed_leaves);

    let leaves_hashes = indices_to_prove
        .iter()
        .map(|idx| hashed_leaves[*idx as usize])
        .collect();

    return (merkle_tree.root().unwrap(), TestData {
        indices_to_prove: indices_to_prove.clone(),
        leaves_hashes,
        proof_bytes: merkle_tree
            .proof(
                indices_to_prove
                    .iter()
                    .map(|x| *x as usize)
                    .collect::<Vec<usize>>()
                    .as_ref(),
            )
            .to_bytes(),
    });
}

fn main() {
    println!("Running merkleproof-trustedroot-native");

    let (merkle_root, proof_data) =
        generate_merkle(vec![21, 32, 101, 201, 1, 2, 3, 90], vec![3, 4]);

    let serialized_proof_bytes = rkyv::to_bytes::<_, 256>(&proof_data).unwrap();

    let files = ["public_input_tape", "private_input_tape"];

    let mut tapes: Vec<std::fs::File> = files
        .iter()
        .map(|x| {
            std::fs::OpenOptions::new()
                .append(true)
                .create(true)
                .open(x)
                .expect("cannot open tape")
        })
        .collect();

    // Write public tape (33 bytes)
    // Function ID (u8)
    tapes[0]
        .write(&[0])
        .expect("(public) write failed for function ID");
    // Merkle state root ([u8; 32])
    tapes[0]
        .write(&merkle_root)
        .expect("(public) write failed for merkle root");

    // Write private tape (variable)
    // Length prefixed (u32)
    tapes[1]
        .write((serialized_proof_bytes.len() as u32).to_le_bytes().as_ref())
        .expect("(private) write failed for merkle proof length prefix");
    // Proof bytes
    tapes[1]
        .write(&serialized_proof_bytes)
        .expect("(private) write failed for merkle proof data");

    // println!("Written test data to {:?}", files);
}

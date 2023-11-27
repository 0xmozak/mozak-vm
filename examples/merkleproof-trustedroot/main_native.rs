// #![feature(restricted_std)]
mod core_logic;

use std::io::Write;

use rs_merkle::algorithms::Sha256;
use rs_merkle::{Hasher, MerkleTree};

use crate::core_logic::TestData;

fn generate_merkle(leaf_values: Vec<u32>, indices_to_prove: Vec<u32>) -> ([u8; 32], TestData) {
    let hashed_leaves: Vec<[u8; 32]> = leaf_values
        .iter()
        .map(|x| Sha256::hash(std::slice::from_ref(x)))
        .collect();

    let merkle_tree = MerkleTree::<Sha256>::from_leaves(&hashed_leaves);

    let leaves_hashes = indices_to_prove
        .iter()
        .map(|idx| hashed_leaves.get(idx.into()).unwrap())
        .collect();

    // = hashed_leaves
    //     .get(indices_to_prove.iter())
    //     .ok_or("can't get leaves to prove")
    //     .unwrap()
    //     .to_vec();

    return (merkle_tree.root().unwrap(), TestData {
        indices_to_prove,
        leaves_hashes,
        proof_bytes: merkle_tree.proof(&indices_to_prove).to_bytes(),
    });
}

fn main() {
    println!("Running merkleproof-trustedroot-native");

    let (merkle_root, proof_data) =
        generate_merkle(vec![21, 32, 101, 201, 1, 2, 3, 90], vec![3, 4]);

    // let leaf_values: Vec<u8> = vec![21, 32, 101, 201, 1, 2, 3, 90];
    // let indices_to_prove: Vec<u8> = vec![3, 4];
    // let leaves: Vec<[u8; 32]> = leaf_values
    //     .iter()
    //     .map(|x| Sha256::hash(std::slice::from_ref(x)))
    //     .collect();

    // let merkle_tree = MerkleTree::<Sha256>::from_leaves(&leaves);
    // let indices_to_prove = vec![3, 4];
    // let leaves_hashes = leaves
    //     .get(3..5)
    //     .ok_or("can't get leaves to prove")
    //     .unwrap()
    //     .to_vec();
    // let merkle_proof = merkle_tree.proof(&indices_to_prove);
    // let merkle_root = merkle_tree.root().unwrap();
    // let proof_bytes = merkle_proof.to_bytes();

    // let private_data = TestData {
    //     indices_to_prove: indices_to_prove.iter().map(|x| *x as u32).collect(),
    //     leaves_hashes,
    //     proof_bytes,
    // };

    let bytes = rkyv::to_bytes::<_, 256>(&proof_data).unwrap();

    let files = ["public_input_tape", "private_input_tape"];

    let mut open_files: Vec<std::fs::File> = files
        .iter()
        .map(|x| {
            std::fs::OpenOptions::new()
                .append(true)
                .create(true)
                .open(x)
                .expect("cannot open tape")
        })
        .collect();

    open_files[0]
        .write(&[0])
        .expect("(public) write failed for function ID");
    open_files[0]
        .write(&merkle_root)
        .expect("(public) write failed for merkle root");
    open_files[1]
        .write(proof_bytes.len().to_le_bytes().as_ref())
        .expect("(private) write failed for merkle proof");

    println!("Written test data to {:?}", files);
}

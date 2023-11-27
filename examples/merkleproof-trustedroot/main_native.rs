// #![feature(restricted_std)]
mod core_logic;

use std::io::{Read, Write};

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

fn serialize_to_disk(files: [&str; 2], merkle_root: [u8; 32], proof_data: &TestData) {
    println!("Serializing and dumping to disk");

    let serialized_proof_bytes = rkyv::to_bytes::<_, 256>(proof_data).unwrap();
    println!("SERLEN: {:?}", serialized_proof_bytes.len());

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

    println!("Serializing and dumping to disk [done]");
}

fn deserialize_from_disk(
    files: [&str; 2],
    expected_merkle_root: [u8; 32],
    expected_testdata: TestData,
) -> ([u8; 32], TestData) {
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
    assert_eq!(merkle_root_buffer, expected_merkle_root, "merkle root mismatched");

    // Read private tape (variable)
    let mut length_prefix = [0u8; 4];
    // Length prefix (u32)
    tapes[1]
        .read(&mut length_prefix)
        .expect("(private) read failed for length prefix");
    
    let length_prefix = u32::from_le_bytes(length_prefix);
    println!("DESLEN: {:?}", length_prefix);

    let mut testdata_buf = Vec::with_capacity(length_prefix as usize);
    testdata_buf.resize(length_prefix as usize, 0);
    tapes[1]
        .read(&mut testdata_buf[0..(length_prefix as usize)])
        .expect("(private) read failed for merkle proof data");

    let deserialized_testdata = unsafe {
        rkyv::from_bytes_unchecked::<TestData>(&testdata_buf)
            .expect("failed to deserialize vec")
    };

    assert_eq!(deserialized_testdata, expected_testdata, "testdata mismatch");
    
    println!("Reading, deserializing and verifying buffers from disk [done]");

    (merkle_root_buffer, TestData::default())
}

fn main() {
    println!("Running merkleproof-trustedroot-native");

    let files = ["public_input_tape", "private_input_tape"];
    let (merkle_root, proof_data) =
        generate_merkle(vec![21, 32, 101, 201, 1, 2, 3, 90], vec![3, 4]);

    serialize_to_disk(files, merkle_root, &proof_data);

    let (merkle_root, proof_data) = deserialize_from_disk(files, merkle_root, proof_data);

    // verify_proof(merkle_root, proof_data);

    println!("all done!")
}

#![no_main]
#![feature(restricted_std)]
mod core_logic;

use std::io::Read;

use mozak_sdk::io::{get_tapes, Extractor};
use rkyv::Deserialize;
use rs_merkle::algorithms::Sha256;
use rs_merkle::MerkleProof;

use crate::core_logic::{ProofData, verify_merkle_proof};

/// ## Function ID 0
/// This function verifies
fn merkleproof_trustedroot_verify(
    // Public inputs
    merkle_root: [u8; 32],

    // Private inputs
    proof_data: ProofData,
) {
    verify_merkle_proof(merkle_root, proof_data);
}

// In general, we try to envision `main()` not be a
// function executing business logic. Instead, we want
// it to be a switch-case for multitude of functions
// executable within the Program.
// In this example, the program to only host one
// function named `public_sum(...)` that verifies
// that two numbers' sum is equivalent to a third number
pub fn main() {
    let (mut public_tape, mut private_tape) = get_tapes();

    let function_id = public_tape.get_u8();

    match function_id {
        0 => {
            let mut merkle_root_buffer = [0u8; 32];
            public_tape
                .read(&mut merkle_root_buffer)
                .expect("(public) read failed for merkle root");

            let mut length_prefix = [0u8; 4];
            // Length prefix (u32)
            private_tape
                .read(&mut length_prefix)
                .expect("(private) read failed for length prefix");
            let length_prefix = u32::from_le_bytes(length_prefix);

            let mut testdata_buf = Vec::with_capacity(length_prefix as usize);
            testdata_buf.resize(length_prefix as usize, 0);
            private_tape
                .read(&mut testdata_buf[0..(length_prefix as usize)])
                .expect("(private) read failed for merkle proof data");

            let archived = unsafe { rkyv::archived_root::<ProofData>(&testdata_buf) };
            let deserialized_testdata: ProofData =
                archived.deserialize(&mut rkyv::Infallible).unwrap();

            merkleproof_trustedroot_verify(merkle_root_buffer, deserialized_testdata);
        }
        _ => (),
    };
}

// We define `main()` to be the program's entry point.
guest::entry!(main);

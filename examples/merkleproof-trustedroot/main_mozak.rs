#![no_main]
#![feature(restricted_std)]

use core::assert_eq;

use mozak_sdk::io::{get_tapes, Extractor};
use rs_merkle::algorithms::Sha256;
use rs_merkle::MerkleProof;

use crate::core_logic::TestData;

/// ## Function ID 0
/// This function verifies
fn merkleproof_trustedroot_verify(
    // Public inputs
    merkleroot: [u8; 32],

    // Private inputs
    leaves: TestData,
    proof: Vec<u8>,
) {
    let proof = MerkleProof::<Sha256>::try_from(proof).unwrap();
    assert!(proof.verify(merkleroot, vec![3, 4], leaf_hashes, leaves.len()))
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
            let mut merkleroot: [u8; 32] = [0; 32];
            const MERKLEPROOF_MAX: usize = 255;
            let mut proofbuf: [u8; MERKLEPROOF_MAX] = [0; MERKLEPROOF_MAX];

            // TODO: make this 32-bit
            let proof_len = private_tape.get_u8();
            public_tape.get_buf(&mut merkleroot, 32);
            private_tape.get_buf(&mut proofbuf, proof_len.into());

            merkleproof_trustedroot_verify(merkleroot, proofbuf[0..(proof_len as usize)].to_vec());
        }
        _ => (),
    };
}

// We define `main()` to be the program's entry point.
guest::entry!(main);

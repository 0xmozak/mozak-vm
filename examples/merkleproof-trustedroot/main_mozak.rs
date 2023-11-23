#![no_main]
#![feature(restricted_std)]

use core::assert_eq;

use mozak_sdk::io::{get_tapes, Extractor};

/// ## Function ID 0
/// This function verifies
fn merkleproof_trustedroot_verify(
    // Public inputs
    merkleroot: [u8; 32],

    // Private inputs
    proof: Vec<u8>,
) {
    assert_eq!(proof.len(), merkleroot.len())
    // assert_eq!(pub_num_1 + pub_num_2, pub_claimed_sum);
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

            let proof_len = private_tape.get_u8();
            public_tape.get_buf(&mut merkleroot, 32);
            private_tape.get_buf(proofbuf, proof_len);

            merkleproof_trustedroot_verify(
                merkleroot,
                Vec::from(proofbuf)[0..proof_len].into()
            );
        }
        _ => (),
    };
}

// We define `main()` to be the program's entry point.
guest::entry!(main);

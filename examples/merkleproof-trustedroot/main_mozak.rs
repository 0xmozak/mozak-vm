#![no_main]
#![feature(restricted_std)]
mod core_logic;

use mozak_sdk::io::{
    from_tape_deserialized, from_tape_rawbuf, get_tapes, MozakPrivateInput, MozakPublicInput,
};

use crate::core_logic::{verify_merkle_proof, MerkleRootType, ProofData};

/// ## Function ID 0
/// This function verifies merkle proof
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

    #[allow(clippy::single_match)]
    match public_tape.get_function_id() {
        0 => {
            // Public tape
            let merkle_root: MerkleRootType =
                from_tape_rawbuf::<MozakPublicInput, 32>(&mut public_tape);

            // Private tape
            let proof_data =
                from_tape_deserialized::<MozakPrivateInput, ProofData, 256>(&mut private_tape);

            merkleproof_trustedroot_verify(merkle_root, proof_data);
        }
        _ => (),
    };
}

// We define `main()` to be the program's entry point.
guest::entry!(main);

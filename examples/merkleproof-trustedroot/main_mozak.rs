#![no_main]
#![feature(restricted_std)]

use core::assert_eq;

use mozak_sdk::io::{get_tapes, Extractor};

/// ## Function ID 0
/// This function verifies
fn merkleproof_trustedroot_verify(
    // Public inputs
    pub_num_1: u8,
    pub_num_2: u8,
    pub_claimed_sum: u8,
) {
    assert_eq!(pub_num_1 + pub_num_2, pub_claimed_sum);
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
        0 => merkleproof_trustedroot_verify(
            public_tape.get_u8(), // Read IO-Tape
            public_tape.get_u8(), // Read IO-Tape
            public_tape.get_u8(), // Read IO-Tape
        ),
        _ => (),
    };
}

// We define `main()` to be the program's entry point.
guest::entry!(main);

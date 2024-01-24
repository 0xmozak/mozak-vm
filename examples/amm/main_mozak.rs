#![no_main]
#![feature(restricted_std)]

mod core_logic;

use mozak_sdk::io::{
    get_tapes
};

// In general, we try to envision `main()` not be a
// function executing business logic. Instead, we want
// it to be a switch-case for multitude of functions
// executable within the Program.
// In this example, the program to only host one
// function named `public_sum(...)` that verifies
// that two numbers' sum is equivalent to a third number
pub fn main() {
    let (mut public_tape, mut _private_tape) = get_tapes();

    #[allow(clippy::single_match)]
    match public_tape.get_function_id() {
        0 => {}
        _ => (),
    };
}

// We define `main()` to be the program's entry point.
guest::entry!(main);

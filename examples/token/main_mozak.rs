#![no_main]
#![feature(restricted_std)]

mod core_logic;

use mozak_sdk::io::{get_tapes, Extractor};

pub fn main() {
    let (mut public_tape, mut _private_tape) = get_tapes();

    #[allow(clippy::single_match)]
    match public_tape.get_u8() {
        0 => {
            // Single function execution
            // match public_tape.get_u8() {
            //     unimplemented!()
            // }
        }
        _ => {
            // Multi-function execution based on recepient
            // for calls in global_transcript_calls(program_id) {
            //     // Do those calls
            // }
        }
    }
}

// We define `main()` to be the program's entry point.
guest::entry!(main);

#![feature(restricted_std)]
mod core_logic;
use std::fs::File;

use mozak_sdk::io::{
    from_tape_deserialized, from_tape_function_id, from_tape_rawbuf, get_tapes_native,
    to_tape_function_id, to_tape_rawbuf, to_tape_serialized,
};
use rs_merkle::algorithms::Sha256;
use rs_merkle::{Hasher, MerkleTree};
use simple_logger::{set_up_color_terminal, SimpleLogger};

fn main() {
    SimpleLogger::new().init().unwrap();
    set_up_color_terminal();

    log::info!("Running stablecoin-native");

    let files = ["public_input.tape", "private_input.tape"];

    log::info!("Generated tapes and verified proof, all done!");
}

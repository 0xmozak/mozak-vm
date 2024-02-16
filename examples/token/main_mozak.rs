#![no_main]
#![feature(restricted_std)]

mod core_logic;

use core;
use std::path::{Path, PathBuf};

use mozak_sdk::coretypes::{CPCMessage, ProgramIdentifier};
use mozak_sdk::sys::call_receive;

pub fn main() {
    if let Some(message) = call_receive() {
        if message.0.caller_prog != ProgramIdentifier::default() {
            panic!("Caller is not the null program");
        };
    }

    guest::env::write(b"1");
}

// We define `main()` to be the program's entry point.
guest::entry!(main);

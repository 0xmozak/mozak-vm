#![no_main]
#![feature(restricted_std)]

mod core_logic;

use std::path::{Path, PathBuf};

use mozak_sdk::coretypes::ProgramIdentifier;
use mozak_sdk::io::{get_tapes, Extractor};
use mozak_sdk::sys::{call_receive, SystemTapes};

pub fn main() {
    assert_eq!(1, 1);
    guest::env::write(b"1");
    //    SystemTapes::load_from_file(Path::new("wallet_tfr.tape_bin"));
    //
    //    if let Some(message) = call_receive() {
    //        if message.0.callee_prog != ProgramIdentifier::default() {
    //            panic!("Program identifiers do not match");
    //        };
    //    }
}

// We define `main()` to be the program's entry point.
guest::entry!(main);

#![no_main]
#![feature(restricted_std)]

mod core_logic;

use std::io::{stdin, BufReader, Read};

use crate::core_logic::MozakIo;

pub fn main() {
    let mut mozak_io = MozakIo {
        stdin: Box::new(BufReader::new(stdin())),
    };
    let mut buffer = [0_u8; 1];
    let n = mozak_io.read(buffer.as_mut()).expect("READ failed");
    assert!(buffer[0] != 0);
    assert!(n == 1);
    guest::env::write(&n.to_be_bytes());
}

guest::entry!(main);

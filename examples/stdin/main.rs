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
    assert!(n == 1);
    let mut buffer = [0_u8; 5];
    let n = mozak_io.read(buffer.as_mut()).expect("READ failed");
    assert!(n == 5);
    let mut buffer = [0_u8; 10]; // requesting more bytes than native run
    let n = mozak_io.read(buffer.as_mut()).expect("READ failed");
    assert!(n == 5); // but iotape generated with native run has total 11 bytes only
    let mut buffer = [0_u8; 1];
    // by now nothing left on iotape so read below must return 0 bytes
    let n = mozak_io.read(buffer.as_mut()).expect("READ failed");
    assert!(n == 0);
    guest::env::write(&n.to_be_bytes());
}

guest::entry!(main);

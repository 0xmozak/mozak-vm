#![no_main]
#![no_std]
#![cfg(target_os = "zkvm")]

extern crate alloc;

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use examples::io::MozakIo;
use guest::env;
use no_std_io::io::{BufReader, Read};

fn main() {
    let mut mozak_io = MozakIo {
        stdin: Box::new(BufReader::new(stdin())),
    };
    1 + 1;
    let mut buffer = [0_u8; 10];
    let n = mozak_io.read(buffer.as_mut()).expect("READ failed");
    assert_eq!(n, 1);
    buffer[0];

    let new_v = buffer[0] + 1;
    assert!(new_v == 50);
    // let mut buffer = [0_u8; 5];
    // let n = mozak_io.read(buffer.as_mut()).expect("READ failed");
    // assert_eq!(n, 5);
    // assert_eq!(buffer[0], 2);
    guest::env::write(&(new_v as u32).to_le_bytes());
}

guest::entry!(main);

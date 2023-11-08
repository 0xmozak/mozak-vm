#![no_main]
#![feature(restricted_std)]

mod core_logic;

use std::io::{stdin, BufReader, Read};

use crate::core_logic::{MozakIoPrivate, MozakIoPublic};

pub fn main() {
    // Private IO
    let mut mozak_io_private = MozakIoPrivate {
        stdin: Box::new(BufReader::new(stdin())),
    };
    let mut buffer = [0_u8; 1];
    let n = mozak_io_private
        .read(buffer.as_mut())
        .expect("Private READ failed");
    assert!(n == 1);
    let mut buffer = [0_u8; 5];
    let n = mozak_io_private
        .read(buffer.as_mut())
        .expect("Private READ failed");
    assert!(n == 5);
    let mut buffer = [0_u8; 5];
    let n = mozak_io_private
        .read(buffer.as_mut())
        .expect("Private READ failed");
    assert!(n == 5);
    guest::env::write(&n.to_be_bytes());

    // Public IO
    let mut mozak_io_public = MozakIoPublic {
        stdin: Box::new(BufReader::new(stdin())),
    };
    let mut buffer = [0_u8; 1];
    let n = mozak_io_public
        .read(buffer.as_mut())
        .expect("Public READ failed");
    assert!(n == 1);
    let mut buffer = [0_u8; 5];
    let n = mozak_io_public
        .read(buffer.as_mut())
        .expect("Public READ failed");
    assert!(n == 5);
    let mut buffer = [0_u8; 5];
    let n = mozak_io_public
        .read(buffer.as_mut())
        .expect("Public READ failed");
    assert!(n == 5);
    guest::env::write(&n.to_be_bytes());
}

guest::entry!(main);

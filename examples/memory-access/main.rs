#![no_main]
#![feature(restricted_std)]

use std::vec::Vec;

pub fn main() {
    let vector: Vec<u8> = vec![128];
    guest::env::write(vector.as_slice());
}

guest::entry!(main);

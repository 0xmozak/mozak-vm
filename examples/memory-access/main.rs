#![no_main]
#![feature(restricted_std)]

use std::vec::Vec;

pub fn main() {
    let mut vector : Vec<u8> = Vec::new();
    vector.push(128);
    guest::env::write(vector.as_slice());
}

guest::entry!(main);

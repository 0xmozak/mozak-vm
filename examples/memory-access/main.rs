#![no_main]
#![no_std]

extern crate alloc;
use alloc::vec;
use alloc::vec::Vec;

pub fn main() {
    let vector: Vec<u8> = vec![128];
    guest::env::write(vector.as_slice());
}

guest::entry!(main);

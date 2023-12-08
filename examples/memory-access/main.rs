#![cfg_attr(target_os = "zkvm", no_main)]
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;
use alloc::vec;
use alloc::vec::Vec;

pub fn main() {
    let vector: Vec<u8> = vec![128];
    guest::env::write(vector.as_slice());
}

guest::entry!(main);

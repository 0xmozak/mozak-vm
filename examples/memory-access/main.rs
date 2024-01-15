#![cfg_attr(target_os = "zkvm", no_main)]
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;
use alloc::vec;
use alloc::vec::Vec;

pub fn main() {
    let mut vector: Vec<u8> = vec![128, 129, 100];
    let mut drain = vector.drain(..);
    guest::env::write(&drain.next().unwrap().to_be_bytes());
}

guest::entry!(main);

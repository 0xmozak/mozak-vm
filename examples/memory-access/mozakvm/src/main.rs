#![cfg_attr(target_os = "mozakvm", no_main)]
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(feature = "std", feature(restricted_std))]

extern crate alloc;
use alloc::vec;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

pub fn main() {
    let mut vector: Vec<u8> = vec![128, 129, 100];
    let mut drain = vector.drain(..);
    mozak_sdk::core::env::write(&drain.next().unwrap().to_be_bytes());
}

mozak_sdk::entry!(main);

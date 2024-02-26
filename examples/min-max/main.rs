#![cfg_attr(target_os = "mozakvm", no_main)]
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;
use alloc::vec::Vec;

pub fn min_max() -> Vec<u8> {
    let min = core::cmp::min(100_u32, 1000_u32);
    let max = core::cmp::max(100_u32, 1000_u32);
    assert!(min < max);
    max.to_be_bytes().to_vec()
}

pub fn main() {
    assert!(false);
    let result = min_max();
    guest::env::write(&result);
}

guest::entry!(main);

#![no_main]
#![feature(restricted_std)]

use core::assert;

pub fn main() {
    let min = std::cmp::min(100_u32, 1000_u32);
    let max = std::cmp::max(100_u32, 1000_u32);
    assert!(min < max);
    guest::env::write(&max.to_be_bytes());
}

guest::entry!(main);

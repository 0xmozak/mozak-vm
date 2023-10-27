#![no_main]
#![no_std]

use core::{assert, assert_eq};

fn fibonacci(n: u32) -> (u32, u32) {
    if n < 2 {
        return (0, n);
    }
    let (mut curr, mut last) = (1_u64, 0_u64);
    for _i in 0..(n - 2) {
        (curr, last) = (curr + last, curr);
    }
    ((curr >> 32) as u32, curr as u32)
}

pub fn main() {
    let (high, low) = fibonacci(40);
    assert!(low == 63245986);
    assert_eq!(high, 0);
    guest::env::write(&high.to_le_bytes());
    assert!(high == 1);
    guest::env::write(&high.to_le_bytes());
}

guest::entry!(main);

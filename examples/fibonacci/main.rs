#![no_main]
#![feature(restricted_std)]

use core::{assert, assert_eq};

fn fibonacci(n: u32) -> (u32, u32) {
    if n == 0 {
        return (0, 0);
    }
    if n == 1 {
        return (0, 1);
    }
    let mut sum = 0_u64;
    let mut last = 0;
    let mut curr = 1;
    for _i in 0..(n - 2) {
        sum = last + curr;
        last = curr;
        curr = sum;
    }
    ((sum >> 32) as u32, sum as u32)
}

pub fn main() {
    let (high, low) = fibonacci(40);
    assert!(low == 63245986);
    assert_eq!(high, 0);
    guest::env::write(&high.to_le_bytes());
}

guest::entry!(main);

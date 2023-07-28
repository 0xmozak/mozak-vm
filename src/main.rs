#![feature(restricted_std)]

use core::assert_eq;

pub fn main() {
    let a = 10;
    let b = a * 10;
    assert_eq!(b, a * 10);
}

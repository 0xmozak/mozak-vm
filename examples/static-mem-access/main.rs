#![no_main]
#![feature(restricted_std)]

use core::assert;

const R_CONST_A: u32 = 41;
static mut R_STATIC_B: u32 = 51;

pub fn main() {
    unsafe {
        assert!(R_CONST_A == 41);
        assert!(R_STATIC_B > 0);
        R_STATIC_B = 56;
        guest::env::write(&R_STATIC_B.to_be_bytes());
    }
}

guest::entry!(main);

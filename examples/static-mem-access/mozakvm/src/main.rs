#![cfg_attr(target_os = "mozakvm", no_main)]
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(feature = "std", feature(restricted_std))]

use core::assert;

const R_CONST_A: u32 = 41;
static mut R_STATIC_B: u32 = 51;

#[allow(clippy::assertions_on_constants)]
pub fn main() {
    unsafe {
        assert!(R_CONST_A == 41);
        assert!(R_STATIC_B > 0);
        R_STATIC_B = 56;
        mozak_sdk::core::env::write(&R_STATIC_B.to_be_bytes());
    }
}

mozak_sdk::entry!(main);

#![cfg_attr(target_os = "mozakvm", no_main)]
#![cfg_attr(not(feature = "std"), no_std)]
use core::assert_eq;

// For BSS / SBSS
// ref: https://stackoverflow.com/questions/40465933/how-do-i-write-rust-code-that-places-globals-statics-in-a-populated-bss-segmen
static mut XYZ: [u8; 20] = [51; 20];

#[allow(static_mut_refs)]
pub fn main() {
    unsafe {
        assert_eq!(XYZ[2], 51);
        mozak_sdk::core::env::write(&XYZ);
    }
}

mozak_sdk::entry!(main);

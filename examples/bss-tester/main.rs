#![cfg_attr(target_os = "zkvm", no_main)]
#![cfg_attr(not(feature = "std"), no_std)]
use core::assert_eq;

// For BSS / SBSS
// ref: https://stackoverflow.com/questions/40465933/how-do-i-write-rust-code-that-places-globals-statics-in-a-populated-bss-segmen
static mut XYZ: [u8; 20] = [51; 20];

pub fn main() {
    unsafe {
        assert_eq!(XYZ[2], 51);
        guest::env::write(&XYZ);
    }
}

guest::entry!(main);

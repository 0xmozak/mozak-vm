#![cfg_attr(target_os = "mozakvm", no_main)]
#![cfg_attr(not(feature = "std"), no_std)]
// #![feature(core_intrinsics)]

// use core::intrinsics;

pub fn main() {
    // intrinsics::abort();
    panic!();
}

mozak_sdk::entry!(main);

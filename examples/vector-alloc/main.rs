#![no_main]
#![feature(restricted_std)]

use core::hint::black_box;

use mozak_sdk::core::ecall::ioread_public;

fn main() {
    let n = {
        let mut bytes = [0u8; 4];
        ioread_public(&mut bytes);
        u32::from_le_bytes(bytes)
    };

    let _v: Vec<u32> = black_box(vec![0; n as usize]);
}

mozak_sdk::entry!(main);

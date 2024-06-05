#![cfg_attr(target_os = "mozakvm", no_main)]
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

#[cfg(target_os = "mozakvm")]
use {alloc::vec, alloc::vec::Vec, core::hint::black_box, mozak_sdk::core::ecall::ioread_public};

#[cfg(target_os = "mozakvm")]
fn alloc_me() {
    let n = {
        let mut bytes = [0u8; 4];
        ioread_public(&mut bytes);
        u32::from_le_bytes(bytes)
    };

    let _v: Vec<u32> = black_box(vec![0; n as usize]);
}

fn main() {
    #[cfg(target_os = "mozakvm")]
    alloc_me();
}

mozak_sdk::entry!(main);

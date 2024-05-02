#![cfg_attr(target_os = "mozakvm", no_main)]
#![cfg_attr(not(feature = "std"), no_std)]

use core::hint::black_box;
extern crate alloc;
use alloc::vec::Vec;

#[cfg(target_os = "mozakvm")]
use mozak_sdk::core::ecall::ioread_public;

extern crate rand;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

#[allow(clippy::unit_arg)]
fn main() {
    let mut rng = black_box(SmallRng::seed_from_u64(0xdead_beef_feed_cafe));

    let n = {
        let mut bytes = [0u8; 4];
        #[cfg(target_os = "mozakvm")]
        ioread_public(bytes.as_mut_ptr(), bytes.len());
        u32::from_le_bytes(bytes)
    };

    let mut v: Vec<u32> = (0..n).map(|_| black_box(rng.gen())).collect();

    black_box(v.sort());
}

mozak_sdk::entry!(main);

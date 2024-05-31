#![cfg_attr(target_os = "mozakvm", no_main)]
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

#[cfg(target_os = "mozakvm")]
use {
    alloc::vec::Vec,
    core::hint::black_box,
    mozak_sdk::core::ecall::ioread_public,
    rand::rngs::SmallRng,
    rand::{Rng, SeedableRng},
};

extern crate rand;

#[allow(clippy::unit_arg)]
#[cfg(target_os = "mozakvm")]
fn sort() {
    let mut rng = black_box(SmallRng::seed_from_u64(0xdead_beef_feed_cafe));

    let n = {
        let mut bytes = [0u8; 4];
        ioread_public(&mut bytes);
        u32::from_le_bytes(bytes)
    };

    let mut v: Vec<u32> = (0..n).map(|_| black_box(rng.gen())).collect();

    black_box(v.sort());
}

fn main() {
    #[cfg(target_os = "mozakvm")]
    sort();
}

mozak_sdk::entry!(main);

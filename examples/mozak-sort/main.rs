#![cfg_attr(target_os = "mozakvm", no_main)]
#![cfg_attr(not(feature = "std"), no_std)]

use core::hint::black_box;
extern crate alloc;
use alloc::vec::Vec;

extern crate rand;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

fn main() {
    // let mut rng = rand::thread_rng();
    let mut rng = SmallRng::seed_from_u64(0xdead_beef_feed_cafe);

    // TODO: take n from tape.
    let n = 100;
    let mut v: Vec<u32> = (0..n).map(|_| black_box(rng.gen())).collect();

    black_box(v.sort());
}

mozak_sdk::entry!(main);
